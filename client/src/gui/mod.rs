//! Overlay ClickGUI rendered inside the glXSwapBuffers hook using egui + egui_glow.
//!
//! Initialised lazily on the first render-thread call so the OpenGL context is
//! guaranteed to be current.

use std::{
    collections::BTreeMap,
    ffi::CString,
    sync::{Arc, Mutex, OnceLock},
};

use egui::Context;
use glow::HasContext;
use log::error;

use crate::{
    glfw::{self, Glfw},
    hotkeys,
    jvm::Jvm,
    lua_engine::{self, ModuleInfo},
    mc::{minecraft::Minecraft, window::get_glfw_window},
};

// ── State ─────────────────────────────────────────────────────────────────────

pub struct ClickGui {
    ctx: Context,
    painter: egui_glow::Painter,
    gl: Arc<glow::Context>,
    glfw: Arc<Glfw>,
    window_ptr: *mut libc::c_void,

    pub visible: bool,
    toggle_key: i32,
    prev_toggle_state: bool,
    binding: bool,

    settings_open: bool,
    scripts_open: bool,
    new_script_buf: String,
    script_error: Option<String>,

    prev_mouse_down: bool,
}

// Safety: only ever accessed from the render thread (glXSwapBuffers hook).
unsafe impl Send for ClickGui {}

static GUI: OnceLock<Mutex<Option<ClickGui>>> = OnceLock::new();

// ── Public API ────────────────────────────────────────────────────────────────

pub fn frame() {
    let container = GUI.get_or_init(|| Mutex::new(None));
    let mut lock = match container.try_lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    if lock.is_none() {
        match ClickGui::new() {
            Ok(gui) => *lock = Some(gui),
            Err(e) => {
                error!("ClickGui init: {:#}", e);
                return;
            }
        }
    }

    if let Some(gui) = lock.as_mut() {
        gui.frame();
    }
}

// ── ClickGui ──────────────────────────────────────────────────────────────────

impl ClickGui {
    fn new() -> anyhow::Result<Self> {
        let gl = unsafe {
            let gl_ctx = glow::Context::from_loader_function(|name| {
                let cstr = CString::new(name).unwrap();
                type GlxGetProcFn = unsafe extern "C" fn(*const u8) -> *const libc::c_void;
                let sym = libc::dlsym(
                    libc::RTLD_DEFAULT,
                    b"glXGetProcAddressARB\0".as_ptr() as *const libc::c_char,
                );
                if !sym.is_null() {
                    let get: GlxGetProcFn = std::mem::transmute(sym);
                    let p = get(cstr.as_ptr() as *const u8);
                    if !p.is_null() {
                        return p as *const _;
                    }
                }
                libc::dlsym(libc::RTLD_DEFAULT, cstr.as_ptr()) as *const _
            });
            Arc::new(gl_ctx)
        };

        unsafe { gl.bind_framebuffer(glow::FRAMEBUFFER, None) };

        let painter = egui_glow::Painter::new(gl.clone(), "", None, false)
            .map_err(|e| anyhow::anyhow!("painter: {}", e))?;

        let glfw = unsafe { Glfw::load()? };

        let window_ptr = {
            let mut env = Jvm::get().attach()?;
            let mc = Minecraft::get_instance(&mut env)?
                .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
            get_glfw_window(&mc, &mut env)?
        };

        // Hotkey thread shares the same Glfw handle and window pointer.
        hotkeys::start(glfw.clone(), window_ptr as usize);

        Ok(ClickGui {
            ctx: Context::default(),
            painter,
            gl,
            glfw,
            window_ptr,
            visible: false,
            toggle_key: glfw::KEY_RIGHT_SHIFT,
            prev_toggle_state: false,
            binding: false,
            settings_open: false,
            scripts_open: false,
            new_script_buf: String::new(),
            script_error: None,
            prev_mouse_down: false,
        })
    }

    fn frame(&mut self) {
        self.handle_toggle();
        if !self.visible {
            return;
        }

        self.glfw.show_cursor(self.window_ptr);

        let (w, h) = unsafe {
            let mut vp = [0i32; 4];
            self.gl.get_parameter_i32_slice(glow::VIEWPORT, &mut vp);
            (vp[2].max(1) as f32, vp[3].max(1) as f32)
        };

        let (mx, my) = self.glfw.cursor_pos(self.window_ptr);
        let mouse_down = self.glfw.mouse_left_down(self.window_ptr);
        let mouse_pressed = mouse_down && !self.prev_mouse_down;
        let mouse_released = !mouse_down && self.prev_mouse_down;
        self.prev_mouse_down = mouse_down;

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, h),
            )),
            events: {
                let mut ev =
                    vec![egui::Event::PointerMoved(egui::pos2(mx as f32, my as f32))];
                if mouse_pressed {
                    ev.push(egui::Event::PointerButton {
                        pos: egui::pos2(mx as f32, my as f32),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    });
                }
                if mouse_released {
                    ev.push(egui::Event::PointerButton {
                        pos: egui::pos2(mx as f32, my as f32),
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    });
                }
                ev
            },
            ..Default::default()
        };

        if self.binding {
            if let Some(key) = self.glfw.scan_any_pressed(self.window_ptr) {
                self.toggle_key = key;
                self.binding = false;
            }
        }

        let modules = lua_engine::get_module_list();
        let categories = group_by_category(modules);

        let ctx = self.ctx.clone();
        let full_output = ctx.run(raw_input, |ctx| {
            self.draw_modules(ctx, &categories);
            self.draw_top_bar(ctx);
            if self.settings_open {
                self.draw_settings(ctx);
            }
            if self.scripts_open {
                self.draw_scripts(ctx);
            }
        });

        unsafe { self.gl.bind_framebuffer(glow::FRAMEBUFFER, None) };

        let clipped = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        self.painter.paint_and_update_textures(
            [w as u32, h as u32],
            full_output.pixels_per_point,
            &clipped,
            &full_output.textures_delta,
        );
    }

    fn handle_toggle(&mut self) {
        let pressed = self.glfw.key_pressed(self.window_ptr, self.toggle_key);
        if pressed && !self.prev_toggle_state {
            self.visible = !self.visible;
        }
        self.prev_toggle_state = pressed;
    }

    // ── UI ────────────────────────────────────────────────────────────────────

    fn draw_modules(&self, ctx: &Context, categories: &BTreeMap<String, Vec<ModuleInfo>>) {
        for (i, (cat_name, modules)) in categories.iter().enumerate() {
            let default_x = 10.0 + i as f32 * 175.0;

            egui::Window::new(cat_name.as_str())
                .id(egui::Id::new(format!("cat_{}", cat_name)))
                .default_pos([default_x, 40.0])
                .resizable(false)
                .collapsible(true)
                .show(ctx, |ui| {
                    ui.set_min_width(160.0);
                    for module in modules {
                        let label = if module.enabled {
                            egui::RichText::new(&module.name)
                                .strong()
                                .color(egui::Color32::from_rgb(140, 220, 140))
                        } else {
                            egui::RichText::new(&module.name)
                        };

                        let resp = ui.add(egui::SelectableLabel::new(module.enabled, label));

                        if resp.clicked() {
                            lua_engine::set_module_enabled(&module.name, !module.enabled);
                        }

                        let hover = if module.description.is_empty() {
                            format!(
                                "Key: {}",
                                if module.key == 0 {
                                    "None".into()
                                } else {
                                    glfw::key_name(module.key)
                                }
                            )
                        } else {
                            format!("{}\nKey: {}", module.description, glfw::key_name(module.key))
                        };
                        resp.on_hover_text(hover);
                    }
                });
        }
    }

    fn draw_top_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("anemoia_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Anemoia")
                        .strong()
                        .color(egui::Color32::from_rgb(180, 100, 255)),
                );
                ui.separator();
                if ui.button("Settings").clicked() {
                    self.settings_open = !self.settings_open;
                }
                if ui.button("Scripts").clicked() {
                    self.scripts_open = !self.scripts_open;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        self.visible = false;
                    }
                });
            });
        });
    }

    fn draw_settings(&mut self, ctx: &Context) {
        let mut open = self.settings_open;
        egui::Window::new("Settings")
            .id(egui::Id::new("anemoia_settings"))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.label("GUI toggle key");
                    ui.horizontal(|ui| {
                        ui.label(glfw::key_name(self.toggle_key));
                        if self.binding {
                            ui.colored_label(egui::Color32::YELLOW, "Press any key...");
                            if ui.button("Cancel").clicked() {
                                self.binding = false;
                            }
                        } else if ui.button("Change").clicked() {
                            self.binding = true;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Module hotkeys are set per-script via the `key` field.",
                    )
                    .weak()
                    .small(),
                );
            });
        self.settings_open = open;
    }

    fn draw_scripts(&mut self, ctx: &Context) {
        let scripts = lua_engine::get_loaded_scripts();
        let mut open = self.scripts_open;

        egui::Window::new("Scripts")
            .id(egui::Id::new("anemoia_scripts"))
            .open(&mut open)
            .min_width(400.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(format!("{} script(s) loaded", scripts.len()));
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for path in &scripts {
                            ui.horizontal(|ui| {
                                ui.label(path);
                                if ui.small_button("[x]").clicked() {
                                    lua_engine::forget_script(path);
                                }
                            });
                        }
                    });

                ui.separator();
                ui.label("Load script:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_script_buf);
                    if ui.button("Load").clicked() {
                        let path = self.new_script_buf.trim().to_owned();
                        if !path.is_empty() {
                            match lua_engine::load_script_file(&path) {
                                Ok(_) => {
                                    self.new_script_buf.clear();
                                    self.script_error = None;
                                }
                                Err(e) => self.script_error = Some(e.to_string()),
                            }
                        }
                    }
                });

                if let Some(err) = &self.script_error {
                    ui.colored_label(egui::Color32::RED, err);
                }
            });

        self.scripts_open = open;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn group_by_category(modules: Vec<ModuleInfo>) -> BTreeMap<String, Vec<ModuleInfo>> {
    let mut map: BTreeMap<String, Vec<ModuleInfo>> = BTreeMap::new();
    for m in modules {
        map.entry(m.category.clone()).or_default().push(m);
    }
    map
}
