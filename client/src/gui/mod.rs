//! Overlay ClickGUI rendered inside the glXSwapBuffers hook using egui + egui_glow.
//!
//! Initialised lazily on the first render-thread call so the OpenGL context is
//! guaranteed to be current.

use std::{
    collections::BTreeMap,
    ffi::CString,
    sync::{Arc, Mutex, OnceLock},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
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

pub static GUI_VISIBLE: AtomicBool = AtomicBool::new(false);
static OLD_CURSOR_POS: AtomicPtr<libc::c_void> = AtomicPtr::new(std::ptr::null_mut());
static OLD_MOUSE_BUTTON: AtomicPtr<libc::c_void> = AtomicPtr::new(std::ptr::null_mut());
static OLD_KEY: AtomicPtr<libc::c_void> = AtomicPtr::new(std::ptr::null_mut());
static OLD_SCROLL: AtomicPtr<libc::c_void> = AtomicPtr::new(std::ptr::null_mut());
static OLD_CHAR: AtomicPtr<libc::c_void> = AtomicPtr::new(std::ptr::null_mut());

// Discrete input events from GLFW callbacks, drained into egui each frame.
static EVENT_QUEUE: Mutex<Vec<egui::Event>> = Mutex::new(Vec::new());
// Last known cursor position, updated by on_cursor_pos for use in on_mouse_button.
static CURSOR_X_BITS: AtomicU64 = AtomicU64::new(0);
static CURSOR_Y_BITS: AtomicU64 = AtomicU64::new(0);

extern "C" fn on_cursor_pos(win: *mut libc::c_void, x: f64, y: f64) {
    CURSOR_X_BITS.store(x.to_bits(), Ordering::Relaxed);
    CURSOR_Y_BITS.store(y.to_bits(), Ordering::Relaxed);
    if !GUI_VISIBLE.load(Ordering::Relaxed) {
        let ptr = OLD_CURSOR_POS.load(Ordering::Relaxed);
        if !ptr.is_null() {
            let cb: glfw::CursorPosCallback = unsafe { std::mem::transmute(ptr) };
            cb(win, x, y);
        }
    }
}

extern "C" fn on_mouse_button(win: *mut libc::c_void, button: i32, action: i32, mods: i32) {
    if GUI_VISIBLE.load(Ordering::Relaxed) {
        let egui_button = match button {
            0 => egui::PointerButton::Primary,
            1 => egui::PointerButton::Secondary,
            2 => egui::PointerButton::Middle,
            _ => return,
        };
        let x = f64::from_bits(CURSOR_X_BITS.load(Ordering::Relaxed));
        let y = f64::from_bits(CURSOR_Y_BITS.load(Ordering::Relaxed));
        if let Ok(mut q) = EVENT_QUEUE.try_lock() {
            q.push(egui::Event::PointerButton {
                pos: egui::pos2(x as f32, y as f32),
                button: egui_button,
                pressed: action != 0,
                modifiers: Default::default(),
            });
        }
    } else {
        let ptr = OLD_MOUSE_BUTTON.load(Ordering::Relaxed);
        if !ptr.is_null() {
            let cb: glfw::MouseButtonCallback = unsafe { std::mem::transmute(ptr) };
            cb(win, button, action, mods);
        }
    }
}

extern "C" fn on_key(win: *mut libc::c_void, key: i32, scancode: i32, action: i32, mods: i32) {
    if GUI_VISIBLE.load(Ordering::Relaxed) {
        if let Some(egui_key) = glfw_key_to_egui(key) {
            let modifiers = egui::Modifiers {
                alt: mods & 0x0004 != 0,
                ctrl: mods & 0x0002 != 0,
                shift: mods & 0x0001 != 0,
                mac_cmd: false,
                command: mods & 0x0002 != 0,
            };
            if let Ok(mut q) = EVENT_QUEUE.try_lock() {
                q.push(egui::Event::Key {
                    key: egui_key,
                    physical_key: None,
                    pressed: action != 0,
                    repeat: action == 2,
                    modifiers,
                });
            }
        }
    } else {
        let ptr = OLD_KEY.load(Ordering::Relaxed);
        if !ptr.is_null() {
            let cb: glfw::KeyCallback = unsafe { std::mem::transmute(ptr) };
            cb(win, key, scancode, action, mods);
        }
    }
}

extern "C" fn on_scroll(win: *mut libc::c_void, xoffset: f64, yoffset: f64) {
    if GUI_VISIBLE.load(Ordering::Relaxed) {
        if let Ok(mut q) = EVENT_QUEUE.try_lock() {
            q.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(xoffset as f32, yoffset as f32),
                modifiers: Default::default(),
            });
        }
    } else {
        let ptr = OLD_SCROLL.load(Ordering::Relaxed);
        if !ptr.is_null() {
            let cb: glfw::ScrollCallback = unsafe { std::mem::transmute(ptr) };
            cb(win, xoffset, yoffset);
        }
    }
}

extern "C" fn on_char(win: *mut libc::c_void, codepoint: u32) {
    if GUI_VISIBLE.load(Ordering::Relaxed) {
        if let Some(ch) = char::from_u32(codepoint) {
            if let Ok(mut q) = EVENT_QUEUE.try_lock() {
                q.push(egui::Event::Text(ch.to_string()));
            }
        }
    } else {
        let ptr = OLD_CHAR.load(Ordering::Relaxed);
        if !ptr.is_null() {
            let cb: glfw::CharCallback = unsafe { std::mem::transmute(ptr) };
            cb(win, codepoint);
        }
    }
}

fn glfw_key_to_egui(key: i32) -> Option<egui::Key> {
    match key {
        256 => Some(egui::Key::Escape),
        257 => Some(egui::Key::Enter),
        258 => Some(egui::Key::Tab),
        259 => Some(egui::Key::Backspace),
        261 => Some(egui::Key::Delete),
        262 => Some(egui::Key::ArrowRight),
        263 => Some(egui::Key::ArrowLeft),
        264 => Some(egui::Key::ArrowDown),
        265 => Some(egui::Key::ArrowUp),
        266 => Some(egui::Key::PageUp),
        267 => Some(egui::Key::PageDown),
        268 => Some(egui::Key::Home),
        269 => Some(egui::Key::End),
        335 => Some(egui::Key::Enter), // KP_ENTER
        65 => Some(egui::Key::A),
        67 => Some(egui::Key::C),
        86 => Some(egui::Key::V),
        88 => Some(egui::Key::X),
        90 => Some(egui::Key::Z),
        _ => None,
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

pub struct ClickGui {
    ctx: Context,
    painter: egui_glow::Painter,
    gl: Arc<glow::Context>,
    glfw: Arc<Glfw>,
    window_ptr: *mut libc::c_void,
    start_time: std::time::Instant,

    visible: bool,
    toggle_key: i32,
    prev_toggle_state: bool,
    binding: bool,
    binding_module: Option<String>,
    binding_module_setting: Option<(String, String)>,
    expanded_module: Option<String>,

    settings_open: bool,

    scripts_open: bool,
    zulip_open: bool,
    new_script_buf: String,
    script_error: Option<String>,

    search_query: String,
}

// Safety: only ever accessed from the render thread (glXSwapBuffers hook).
unsafe impl Send for ClickGui {}

impl Drop for ClickGui {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}

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
        // Fallible non-GL setup first so the painter is never created on error paths.
        let glfw = unsafe { Glfw::load()? };

        let window_ptr = {
            let mut env = Jvm::get().attach()?;
            let mc = Minecraft::get_instance(&mut env)?
                .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
            get_glfw_window(&mc, &mut env)?
        };

        let gl = unsafe {
            let gl_ctx = glow::Context::from_loader_function(|name| {
                let cstr = CString::new(name).unwrap();

                // Try EGL first
                if let Some(egl_get) = crate::hook::find_sym("eglGetProcAddress", "libEGL") {
                    type EglGetProcFn = unsafe extern "C" fn(*const libc::c_char) -> *const libc::c_void;
                    let get: EglGetProcFn = std::mem::transmute(egl_get);
                    let p = get(cstr.as_ptr());
                    if !p.is_null() {
                        return p as *const _;
                    }
                }

                // Fall back to GLX
                if let Some(glx_get) = crate::hook::find_sym("glXGetProcAddressARB", "libGL") {
                    type GlxGetProcFn = unsafe extern "C" fn(*const u8) -> *const libc::c_void;
                    let get: GlxGetProcFn = std::mem::transmute(glx_get);
                    let p = get(cstr.as_ptr() as *const u8);
                    if !p.is_null() {
                        return p as *const _;
                    }
                }

                // Final fallback: try global namespace directly
                libc::dlsym(libc::RTLD_DEFAULT, cstr.as_ptr()) as *const _
            });
            Arc::new(gl_ctx)
        };

        unsafe { gl.bind_framebuffer(glow::FRAMEBUFFER, None) };

        let painter = egui_glow::Painter::new(gl.clone(), "", None, false)
            .map_err(|e| anyhow::anyhow!("painter: {}", e))?;
        // No fallible operations after this point — painter is always stored or destroyed via Drop.

        unsafe {
            if let Some(old) = (glfw.set_cursor_pos_cb)(window_ptr, Some(on_cursor_pos)) {
                OLD_CURSOR_POS.store(old as *mut _, Ordering::Relaxed);
            }
            if let Some(old) = (glfw.set_mouse_button_cb)(window_ptr, Some(on_mouse_button)) {
                OLD_MOUSE_BUTTON.store(old as *mut _, Ordering::Relaxed);
            }
            if let Some(old) = (glfw.set_key_cb)(window_ptr, Some(on_key)) {
                OLD_KEY.store(old as *mut _, Ordering::Relaxed);
            }
            if let Some(old) = (glfw.set_scroll_cb)(window_ptr, Some(on_scroll)) {
                OLD_SCROLL.store(old as *mut _, Ordering::Relaxed);
            }
            if let Some(old) = (glfw.set_char_cb)(window_ptr, Some(on_char)) {
                OLD_CHAR.store(old as *mut _, Ordering::Relaxed);
            }
        }

        // Hotkey thread shares the same Glfw handle and window pointer.
        // Hotkeys are now ticked synchronously in the frame loop.

        Ok(ClickGui {
            ctx: Context::default(),
            painter,
            gl,
            glfw,
            window_ptr,
            start_time: std::time::Instant::now(),
            visible: false,
            toggle_key: glfw::KEY_RIGHT_SHIFT,
            prev_toggle_state: false,
            binding: false,
            binding_module: None,
            binding_module_setting: None,
            expanded_module: None,
            settings_open: false,
            scripts_open: false,
            zulip_open: false,
            new_script_buf: String::new(),
            script_error: None,
            search_query: String::new(),
        })
    }

    fn frame(&mut self) {
        hotkeys::tick(&self.glfw, self.window_ptr);
        self.handle_toggle();
        if !self.visible {
            return;
        }

        self.glfw.show_cursor(self.window_ptr);

        let (fw, fh) = self.glfw.framebuffer_size(self.window_ptr);
        let w = fw.max(1) as f32;
        let h = fh.max(1) as f32;

        let (mx, my) = self.glfw.cursor_pos(self.window_ptr);

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, h),
            )),
            time: Some(self.start_time.elapsed().as_secs_f64()),
            predicted_dt: 1.0 / 60.0,
            events: {
                let mut ev = vec![egui::Event::PointerMoved(egui::pos2(mx as f32, my as f32))];
                if let Ok(mut q) = EVENT_QUEUE.try_lock() {
                    ev.append(&mut q);
                }
                ev
            },
            ..Default::default()
        };

        if self.binding {
            if let Some(key) = self.glfw.scan_any_pressed(self.window_ptr) {
                if let Some((mod_name, setting_name)) = self.binding_module_setting.take() {
                    lua_engine::set_module_setting(&mod_name, &setting_name, mlua::Value::Integer(key.into()));
                } else if let Some(mod_name) = self.binding_module.take() {
                    lua_engine::set_module_key(&mod_name, key);
                } else {
                    self.toggle_key = key;
                }
                self.binding = false;
            }
        }

        let modules = lua_engine::get_module_list();
        let categories = group_by_category(modules);

        let ctx = self.ctx.clone();
        let full_output = ctx.run(raw_input, |ctx| {
            // Draw background-level HUD/ESP from Lua
            let painter = ctx.layer_painter(egui::LayerId::background());
            if let Err(e) = lua_engine::on_render(painter) {
                error!("on_render error: {:#}", e);
            }

            self.draw_top_bar(ctx);
            self.draw_modules(ctx, &categories);
            if self.settings_open {
                self.draw_settings(ctx);
            }
            if self.scripts_open {
                self.draw_scripts(ctx);
            }
            if self.zulip_open {
                self.draw_zulip(ctx);
            }
        });

        let mut last_draw_framebuffer = 0;
        let mut last_read_framebuffer = 0;
        let mut last_program = 0;
        let mut last_texture_0 = 0;
        let mut last_sampler_0 = 0;
        let mut last_active_texture = 0;
        let mut last_array_buffer = 0;
        let mut last_element_array_buffer = 0;
        let mut last_vertex_array = 0;
        let mut last_blend_src_rgb = 0;
        let mut last_blend_dst_rgb = 0;
        let mut last_blend_src_alpha = 0;
        let mut last_blend_dst_alpha = 0;
        let mut last_blend_equation_rgb = 0;
        let mut last_blend_equation_alpha = 0;
        let mut last_viewport = [0i32; 4];
        let mut last_scissor_box = [0i32; 4];
        let mut last_color_mask = [0i32; 4];
        let mut last_polygon_mode = [0i32; 2];
        let mut last_unpack_buffer = 0;
        let mut last_unpack_row_length = 0;
        let mut last_unpack_skip_pixels = 0;
        let mut last_unpack_skip_rows = 0;
        let mut last_unpack_alignment = 4;

        unsafe {
            self.gl.get_parameter_i32_slice(glow::DRAW_FRAMEBUFFER_BINDING, std::slice::from_mut(&mut last_draw_framebuffer));
            self.gl.get_parameter_i32_slice(glow::READ_FRAMEBUFFER_BINDING, std::slice::from_mut(&mut last_read_framebuffer));
            self.gl.get_parameter_i32_slice(glow::CURRENT_PROGRAM, std::slice::from_mut(&mut last_program));
            self.gl.get_parameter_i32_slice(glow::COLOR_WRITEMASK, &mut last_color_mask);
            
            self.gl.get_parameter_i32_slice(glow::ACTIVE_TEXTURE, std::slice::from_mut(&mut last_active_texture));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.get_parameter_i32_slice(glow::TEXTURE_BINDING_2D, std::slice::from_mut(&mut last_texture_0));
            self.gl.get_parameter_i32_slice(glow::SAMPLER_BINDING, std::slice::from_mut(&mut last_sampler_0));
            self.gl.active_texture(last_active_texture as u32);
            
            self.gl.get_parameter_i32_slice(glow::ARRAY_BUFFER_BINDING, std::slice::from_mut(&mut last_array_buffer));
            self.gl.get_parameter_i32_slice(glow::ELEMENT_ARRAY_BUFFER_BINDING, std::slice::from_mut(&mut last_element_array_buffer));
            self.gl.get_parameter_i32_slice(glow::VERTEX_ARRAY_BINDING, std::slice::from_mut(&mut last_vertex_array));
            self.gl.get_parameter_i32_slice(glow::BLEND_SRC_RGB, std::slice::from_mut(&mut last_blend_src_rgb));
            self.gl.get_parameter_i32_slice(glow::BLEND_DST_RGB, std::slice::from_mut(&mut last_blend_dst_rgb));
            self.gl.get_parameter_i32_slice(glow::BLEND_SRC_ALPHA, std::slice::from_mut(&mut last_blend_src_alpha));
            self.gl.get_parameter_i32_slice(glow::BLEND_DST_ALPHA, std::slice::from_mut(&mut last_blend_dst_alpha));
            self.gl.get_parameter_i32_slice(glow::BLEND_EQUATION_RGB, std::slice::from_mut(&mut last_blend_equation_rgb));
            self.gl.get_parameter_i32_slice(glow::BLEND_EQUATION_ALPHA, std::slice::from_mut(&mut last_blend_equation_alpha));
            self.gl.get_parameter_i32_slice(glow::VIEWPORT, &mut last_viewport);
            self.gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut last_scissor_box);
            self.gl.get_parameter_i32_slice(glow::POLYGON_MODE, &mut last_polygon_mode);
            self.gl.get_parameter_i32_slice(glow::PIXEL_UNPACK_BUFFER_BINDING, std::slice::from_mut(&mut last_unpack_buffer));
            self.gl.get_parameter_i32_slice(glow::UNPACK_ROW_LENGTH, std::slice::from_mut(&mut last_unpack_row_length));
            self.gl.get_parameter_i32_slice(glow::UNPACK_SKIP_PIXELS, std::slice::from_mut(&mut last_unpack_skip_pixels));
            self.gl.get_parameter_i32_slice(glow::UNPACK_SKIP_ROWS, std::slice::from_mut(&mut last_unpack_skip_rows));
            self.gl.get_parameter_i32_slice(glow::UNPACK_ALIGNMENT, std::slice::from_mut(&mut last_unpack_alignment));
        }

        let blend_enabled = unsafe { self.gl.is_enabled(glow::BLEND) };
        let cull_face_enabled = unsafe { self.gl.is_enabled(glow::CULL_FACE) };
        let depth_test_enabled = unsafe { self.gl.is_enabled(glow::DEPTH_TEST) };
        let scissor_test_enabled = unsafe { self.gl.is_enabled(glow::SCISSOR_TEST) };
        let stencil_test_enabled = unsafe { self.gl.is_enabled(glow::STENCIL_TEST) };

        unsafe { self.gl.bind_framebuffer(glow::FRAMEBUFFER, None) };

        unsafe {
            self.gl.color_mask(true, true, true, true);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.disable(glow::STENCIL_TEST);
            self.gl.bind_sampler(0, None);
            self.gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            self.gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
            self.gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
            self.gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        }

        let clipped = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        self.painter.paint_and_update_textures(
            [w as u32, h as u32],
            full_output.pixels_per_point,
            &clipped,
            &full_output.textures_delta,
        );

        unsafe {
            self.gl.use_program(std::num::NonZeroU32::new(last_program as u32).map(glow::NativeProgram));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, std::num::NonZeroU32::new(last_texture_0 as u32).map(glow::NativeTexture));
            self.gl.active_texture(last_active_texture as u32);
            self.gl.bind_vertex_array(std::num::NonZeroU32::new(last_vertex_array as u32).map(glow::NativeVertexArray));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, std::num::NonZeroU32::new(last_array_buffer as u32).map(glow::NativeBuffer));
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, std::num::NonZeroU32::new(last_element_array_buffer as u32).map(glow::NativeBuffer));
            
            self.gl.blend_equation_separate(last_blend_equation_rgb as u32, last_blend_equation_alpha as u32);
            self.gl.blend_func_separate(last_blend_src_rgb as u32, last_blend_dst_rgb as u32, last_blend_src_alpha as u32, last_blend_dst_alpha as u32);
            
            if blend_enabled { self.gl.enable(glow::BLEND); } else { self.gl.disable(glow::BLEND); }
            if cull_face_enabled { self.gl.enable(glow::CULL_FACE); } else { self.gl.disable(glow::CULL_FACE); }
            if depth_test_enabled { self.gl.enable(glow::DEPTH_TEST); } else { self.gl.disable(glow::DEPTH_TEST); }
            if scissor_test_enabled { self.gl.enable(glow::SCISSOR_TEST); } else { self.gl.disable(glow::SCISSOR_TEST); }
            if stencil_test_enabled { self.gl.enable(glow::STENCIL_TEST); } else { self.gl.disable(glow::STENCIL_TEST); }
            
            self.gl.viewport(last_viewport[0], last_viewport[1], last_viewport[2], last_viewport[3]);
            self.gl.scissor(last_scissor_box[0], last_scissor_box[1], last_scissor_box[2], last_scissor_box[3]);
            self.gl.color_mask(last_color_mask[0] != 0, last_color_mask[1] != 0, last_color_mask[2] != 0, last_color_mask[3] != 0);
            self.gl.polygon_mode(glow::FRONT_AND_BACK, last_polygon_mode[0] as u32);
            self.gl.bind_sampler(0, std::num::NonZeroU32::new(last_sampler_0 as u32).map(glow::NativeSampler));
            self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, std::num::NonZeroU32::new(last_unpack_buffer as u32).map(glow::NativeBuffer));
            self.gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, last_unpack_row_length);
            self.gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, last_unpack_skip_pixels);
            self.gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, last_unpack_skip_rows);
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack_alignment);
            self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, std::num::NonZeroU32::new(last_draw_framebuffer as u32).map(glow::NativeFramebuffer));
            self.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, std::num::NonZeroU32::new(last_read_framebuffer as u32).map(glow::NativeFramebuffer));
        }
    }

    fn handle_toggle(&mut self) {
        let mut env = match Jvm::get().attach() {
            Ok(e) => e,
            Err(_) => return,
        };

        // Close if a Minecraft screen (like Esc menu) is opened
        if self.visible {
            if let Ok(Some(mc_obj)) = Minecraft::get_instance(&mut env) {
                if let Ok(Some(_)) = mc_obj.get_current_screen(&mut env) {
                    self.visible = false;
                    GUI_VISIBLE.store(false, Ordering::Relaxed);
                    self.glfw.hide_cursor(self.window_ptr);
                }
            }
        }

        let pressed = self.glfw.key_pressed(self.window_ptr, self.toggle_key);
        if pressed && !self.prev_toggle_state {
            self.visible = !self.visible;
            GUI_VISIBLE.store(self.visible, Ordering::Relaxed);
            if !self.visible {
                // Restore hidden cursor
                self.glfw.hide_cursor(self.window_ptr);
            }
        }
        self.prev_toggle_state = pressed;
    }

    // ── UI ────────────────────────────────────────────────────────────────────

    fn draw_modules(&mut self, ctx: &Context, categories: &BTreeMap<String, Vec<ModuleInfo>>) {
        let query = self.search_query.trim().to_lowercase();

        if !query.is_empty() {
            let results: Vec<&ModuleInfo> = categories
                .values()
                .flatten()
                .filter(|m| m.name.to_lowercase().contains(&query))
                .collect();

            egui::Window::new("Search Results")
                .id(egui::Id::new("anemoia_search"))
                .default_pos([10.0, 40.0])
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.set_min_width(200.0);
                    if results.is_empty() {
                        ui.label("No modules match.");
                    }
                    for module in results {
                        self.draw_module_row(ui, module);
                    }
                });
            return;
        }

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
                        self.draw_module_row(ui, module);
                    }
                });
        }
    }

    fn draw_module_row(&mut self, ui: &mut egui::Ui, module: &ModuleInfo) {
        let label = if module.enabled {
            egui::RichText::new(&module.name)
                .strong()
                .color(egui::Color32::from_rgb(140, 220, 140))
        } else {
            egui::RichText::new(&module.name)
        };

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let resp = ui.add(egui::SelectableLabel::new(module.enabled, label));

                if resp.clicked() {
                    lua_engine::set_module_enabled(&module.name, !module.enabled);
                }

                if resp.secondary_clicked() {
                    if self.expanded_module.as_deref() == Some(&module.name) {
                        self.expanded_module = None;
                    } else {
                        self.expanded_module = Some(module.name.clone());
                    }
                }

                let hover = if module.description.is_empty() {
                    "No description (Right-click for settings)".to_owned()
                } else {
                    format!("{}\n(Right-click for settings)", module.description)
                };
                resp.on_hover_text(hover);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let bind_label_buf: String;
                    let bind_label = if self.binding && self.binding_module.as_deref() == Some(&module.name) && self.binding_module_setting.is_none() {
                        "..."
                    } else if module.key == 0 {
                        "None"
                    } else {
                        bind_label_buf = glfw::key_name(module.key);
                        &bind_label_buf
                    };

                    if ui.button(format!("Bind: {}", bind_label)).clicked() {
                        self.binding = true;
                        self.binding_module = Some(module.name.clone());
                        self.binding_module_setting = None;
                    }
                });
            });

            if self.expanded_module.as_deref() == Some(&module.name) {
                ui.indent(format!("indent_{}", module.name), |ui| {
                    for setting in &module.settings {
                        match setting {
                            lua_engine::ModuleSetting::Boolean { name, value } => {
                                let mut b = *value;
                                if ui.checkbox(&mut b, name).changed() {
                                    lua_engine::set_module_setting(&module.name, name, mlua::Value::Boolean(b));
                                }
                            }
                            lua_engine::ModuleSetting::Number { name, value, min, max } => {
                                ui.label(name);
                                let mut v = *value;
                                if ui.add(egui::Slider::new(&mut v, *min..=*max)).changed() {
                                    lua_engine::set_module_setting(&module.name, name, mlua::Value::Number(v));
                                }
                            }
                            lua_engine::ModuleSetting::Keybind { name, value } => {
                                ui.horizontal(|ui| {
                                    ui.label(name);
                                    let bind_label = if self.binding && self.binding_module_setting == Some((module.name.clone(), name.clone())) {
                                        "..."
                                    } else {
                                        &glfw::key_name(*value)
                                    };
                                    if ui.button(bind_label).clicked() {
                                        self.binding = true;
                                        self.binding_module_setting = Some((module.name.clone(), name.clone()));
                                    }
                                });
                            }
                            lua_engine::ModuleSetting::Enum { name, value, options } => {
                                ui.horizontal(|ui| {
                                    ui.label(name);
                                    let mut current = value.clone();
                                    egui::ComboBox::from_id_salt(format!("{}_{}", module.name, name))
                                        .selected_text(&current)
                                        .show_ui(ui, |ui| {
                                            for opt in options {
                                                if ui.selectable_value(&mut current, opt.clone(), opt).changed() {
                                                    lua_engine::set_module_setting(&module.name, name, lua_engine::create_string(&current));
                                                }
                                            }
                                        });
                                });
                            }
                        }
                    }
                });
            }
        });
    }

    fn draw_top_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("anemoia_bar").show(ctx, |ui| {
            ui.add_space(4.0);
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
                if ui.button("Zulip").clicked() {
                    self.zulip_open = !self.zulip_open;
                }
                ui.separator();
                ui.label("Search:");
                let search = egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(140.0)
                    .hint_text("filter modules...");
                ui.add(search);
                if !self.search_query.is_empty() && ui.small_button("[x]").clicked() {
                    self.search_query.clear();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        self.visible = false;
                        GUI_VISIBLE.store(false, Ordering::Relaxed);
                        self.glfw.hide_cursor(self.window_ptr);
                    }
                });
            });
            ui.add_space(4.0);
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

    fn draw_zulip(&mut self, ctx: &Context) {
        let mut open = self.zulip_open;
        egui::Window::new("Zulip Bridge")
            .id(egui::Id::new("anemoia_zulip"))
            .open(&mut open)
            .min_width(400.0)
            .resizable(true)
            .show(ctx, |ui| {
                let painter = ui.painter().clone();
                if let Err(e) = lua_engine::on_zulip_ui(painter) {
                    error!("on_zulip_ui error: {:#}", e);
                }
            });
        self.zulip_open = open;
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

pub fn cleanup() {
    let container = GUI.get_or_init(|| Mutex::new(None));
    if let Ok(mut lock) = container.try_lock() {
        if let Some(gui) = lock.take() {
            unsafe {
                let win = gui.window_ptr;
                let old = OLD_CURSOR_POS.load(Ordering::Relaxed);
                if !old.is_null() {
                    let cb: glfw::CursorPosCallback = std::mem::transmute(old);
                    (gui.glfw.set_cursor_pos_cb)(win, Some(cb));
                } else {
                    (gui.glfw.set_cursor_pos_cb)(win, None);
                }
                
                let old = OLD_MOUSE_BUTTON.load(Ordering::Relaxed);
                if !old.is_null() {
                    let cb: glfw::MouseButtonCallback = std::mem::transmute(old);
                    (gui.glfw.set_mouse_button_cb)(win, Some(cb));
                } else {
                    (gui.glfw.set_mouse_button_cb)(win, None);
                }
                
                let old = OLD_KEY.load(Ordering::Relaxed);
                if !old.is_null() {
                    let cb: glfw::KeyCallback = std::mem::transmute(old);
                    (gui.glfw.set_key_cb)(win, Some(cb));
                } else {
                    (gui.glfw.set_key_cb)(win, None);
                }
                
                let old = OLD_SCROLL.load(Ordering::Relaxed);
                if !old.is_null() {
                    let cb: glfw::ScrollCallback = std::mem::transmute(old);
                    (gui.glfw.set_scroll_cb)(win, Some(cb));
                } else {
                    (gui.glfw.set_scroll_cb)(win, None);
                }
                
                let old = OLD_CHAR.load(Ordering::Relaxed);
                if !old.is_null() {
                    let cb: glfw::CharCallback = std::mem::transmute(old);
                    (gui.glfw.set_char_cb)(win, Some(cb));
                } else {
                    (gui.glfw.set_char_cb)(win, None);
                }
                
                if gui.visible {
                    gui.glfw.hide_cursor(win);
                }
            }
        }
    }
}
