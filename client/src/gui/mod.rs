//! Overlay ClickGUI rendered inside the glXSwapBuffers hook using egui + egui_glow.
//!
//! Initialised lazily on the first render-thread call so the OpenGL context is
//! guaranteed to be current.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
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
    zulip_msg_buf: String,
    script_error: Option<String>,

    packet_mgr_open: bool,

    search_query: String,
    new_profile_buf: String,

    // Category window persistence
    folder_positions: HashMap<String, egui::Pos2>,
    initialized_folders: HashSet<String>,

    // Accent color (RGB)
    accent_color: egui::Color32,
    accent_rgb: [f32; 3],

    // Zulip connection config buffers (also used in global Settings panel)
    zulip_url_buf: String,
    zulip_email_buf: String,
    zulip_key_buf: String,
    zulip_stream_buf: String,
    zulip_topic_buf: String,

    // Full Zulip GUI state
    zulip_in_settings: bool,
    zulip_selected_stream: String,
    zulip_selected_topic: String,
    zulip_expanded_stream: Option<String>,
    zulip_texture_cache: HashMap<String, egui::TextureHandle>,
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

        let cfg = crate::config::get();
        let folder_positions: HashMap<String, egui::Pos2> = cfg.folder_positions
            .iter()
            .map(|(k, v)| (k.clone(), egui::pos2(v[0], v[1])))
            .collect();
        let [ar, ag, ab] = cfg.accent_color;
        let accent_color = egui::Color32::from_rgb(ar, ag, ab);
        let accent_rgb = [ar as f32 / 255.0, ag as f32 / 255.0, ab as f32 / 255.0];

        Ok(ClickGui {
            ctx: Context::default(),
            painter,
            gl,
            glfw,
            window_ptr,
            start_time: std::time::Instant::now(),
            visible: false,
            toggle_key: cfg.gui_toggle_key,
            prev_toggle_state: false,
            binding: false,
            binding_module: None,
            binding_module_setting: None,
            expanded_module: None,
            settings_open: false,
            scripts_open: false,
            zulip_open: false,
            new_script_buf: String::new(),
            zulip_msg_buf: String::new(),
            script_error: None,
            packet_mgr_open: false,
            search_query: String::new(),
            new_profile_buf: String::new(),
            folder_positions,
            initialized_folders: HashSet::new(),
            accent_color,
            accent_rgb,
            zulip_url_buf: String::new(),
            zulip_email_buf: String::new(),
            zulip_key_buf: String::new(),
            zulip_stream_buf: String::new(),
            zulip_topic_buf: String::new(),
            zulip_in_settings: false,
            zulip_selected_stream: String::new(),
            zulip_selected_topic: String::new(),
            zulip_expanded_stream: None,
            zulip_texture_cache: HashMap::new(),
        })
    }

    fn frame(&mut self) {
        // When window leaves focus, hide the GUI cursor/overlay.
        if self.visible && !self.glfw.window_focused(self.window_ptr) {
            self.visible = false;
            GUI_VISIBLE.store(false, Ordering::Relaxed);
            self.glfw.hide_cursor(self.window_ptr);
        }

        // Skip ALL GL/egui work when not focused.  Running egui_glow inside
        // glXSwapBuffers while the window is on another workspace causes the
        // NVIDIA compositor to stall (visible as a freeze) and can corrupt the
        // trampoline page boundary causing a SIGSEGV.
        if !self.glfw.window_focused(self.window_ptr) {
            return;
        }

        hotkeys::tick(&self.glfw, self.window_ptr);
        self.handle_toggle();

        if self.visible {
            self.glfw.show_cursor(self.window_ptr);
        }

        let (fw, fh) = self.glfw.framebuffer_size(self.window_ptr);
        let w = fw.max(1) as f32;
        let h = fh.max(1) as f32;

        let (mx, my) = if self.visible {
            self.glfw.cursor_pos(self.window_ptr)
        } else {
            (-1000.0, -1000.0) // Keep mouse away when not visible
        };

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, h),
            )),
            time: Some(self.start_time.elapsed().as_secs_f64()),
            predicted_dt: 1.0 / 60.0,
            events: {
                let mut ev = if self.visible {
                    vec![egui::Event::PointerMoved(egui::pos2(mx as f32, my as f32))]
                } else {
                    vec![]
                };
                if let Ok(mut q) = EVENT_QUEUE.try_lock() {
                    for e in q.drain(..) {
                        if let egui::Event::Key { key: egui::Key::V, pressed: true, modifiers, .. } = &e {
                            if modifiers.command || modifiers.ctrl {
                                if let Some(s) = self.glfw.get_clipboard(self.window_ptr) {
                                    ev.push(egui::Event::Paste(s));
                                }
                            }
                        }
                        ev.push(e);
                    }
                }
                ev
            },
            ..Default::default()
        };
        if self.visible && self.binding {
            if let Some(key) = self.glfw.scan_any_pressed(self.window_ptr) {
                if let Some((mod_name, setting_name)) = self.binding_module_setting.take() {
                    lua_engine::set_module_setting(&mod_name, &setting_name, mlua::Value::Integer(key.into()));
                } else if let Some(mod_name) = self.binding_module.take() {
                    lua_engine::set_module_key(&mod_name, key);
                } else {
                    self.toggle_key = key;
                    crate::config::modify(|c| c.gui_toggle_key = key);
                }
                self.binding = false;
            }
        }

        self.maybe_reflect_selected();

        let modules = lua_engine::get_module_list();
        let categories = group_by_category(modules);

        let ctx = self.ctx.clone();
        let full_output = ctx.run(raw_input, |ctx| {
            // Draw background-level HUD/ESP from Lua
            let painter = ctx.layer_painter(egui::LayerId::background());
            if let Err(e) = lua_engine::on_render(painter) {
                error!("on_render error: {:#}", e);
            }

            if self.visible {
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
                if self.packet_mgr_open {
                    self.draw_packet_mgr(ctx);
                }
            }
        });

        if !full_output.platform_output.copied_text.is_empty() {
            self.glfw.set_clipboard(self.window_ptr, &full_output.platform_output.copied_text);
        }

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
        let pressed = self.glfw.key_pressed(self.window_ptr, self.toggle_key);
        if pressed && !self.prev_toggle_state {
            self.visible = !self.visible;
            GUI_VISIBLE.store(self.visible, Ordering::Relaxed);
            if !self.visible {
                self.glfw.hide_cursor(self.window_ptr);
                self.save_folder_positions();
            } else {
                // Re-opening: force egui to restore our saved positions on first frame
                self.initialized_folders.clear();
            }
        }
        self.prev_toggle_state = pressed;

        if self.visible && self.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.visible = false;
            GUI_VISIBLE.store(false, Ordering::Relaxed);
            self.glfw.hide_cursor(self.window_ptr);
            self.save_folder_positions();
        }
    }

    fn save_folder_positions(&self) {
        let positions: HashMap<String, [f32; 2]> = self.folder_positions
            .iter()
            .map(|(k, v)| (k.clone(), [v.x, v.y]))
            .collect();
        crate::config::modify(|c| c.folder_positions = positions);
    }

    fn reload_from_config(&mut self) {
        let cfg = crate::config::get();
        self.toggle_key = cfg.gui_toggle_key;
        let [r, g, b] = cfg.accent_color;
        self.accent_color = egui::Color32::from_rgb(r, g, b);
        self.accent_rgb = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0];
        self.folder_positions = cfg.folder_positions
            .iter()
            .map(|(k, v)| (k.clone(), egui::pos2(v[0], v[1])))
            .collect();
        self.initialized_folders.clear();
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
            let default_pos = egui::pos2(default_x, 40.0);
            let saved_pos = self.folder_positions.get(cat_name).copied();
            // first_time == true means this is the first frame we see this window this session
            let first_time = self.initialized_folders.insert(cat_name.clone());

            let win = egui::Window::new(cat_name.as_str())
                .id(egui::Id::new(format!("cat_{}", cat_name)))
                .resizable(false)
                .collapsible(true);

            // On first appearance, force egui to use our saved position.
            // After that, egui's own memory tracks drags; we just observe.
            let win = if first_time {
                win.current_pos(saved_pos.unwrap_or(default_pos))
            } else {
                win.default_pos(saved_pos.unwrap_or(default_pos))
            };

            let resp = win.show(ctx, |ui| {
                ui.set_min_width(160.0);
                for module in modules {
                    self.draw_module_row(ui, module);
                }
            });

            if let Some(ir) = resp {
                self.folder_positions.insert(cat_name.clone(), ir.response.rect.min);
            }
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
                        .color(self.accent_color),
                );
                ui.separator();
                if ui.button("Settings").clicked() {
                    self.settings_open = !self.settings_open;
                    if self.settings_open {
                        let cfg = crate::zulip::get_config();
                        self.zulip_url_buf = cfg.url;
                        self.zulip_email_buf = cfg.email;
                        self.zulip_key_buf = cfg.api_key;
                        self.zulip_stream_buf = cfg.stream;
                        self.zulip_topic_buf = cfg.topic;
                    }
                }
                if ui.button("Scripts").clicked() {
                    self.scripts_open = !self.scripts_open;
                }
                if ui.button("Zulip").clicked() {
                    self.zulip_open = !self.zulip_open;
                }
                if ui.button("Packets").clicked() {
                    self.packet_mgr_open = !self.packet_mgr_open;
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
                    ui.label(egui::RichText::new("General").strong());
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
                    ui.separator();
                    ui.label("Accent Color");
                    if ui.color_edit_button_rgb(&mut self.accent_rgb).changed() {
                        let r = (self.accent_rgb[0] * 255.0) as u8;
                        let g = (self.accent_rgb[1] * 255.0) as u8;
                        let b = (self.accent_rgb[2] * 255.0) as u8;
                        self.accent_color = egui::Color32::from_rgb(r, g, b);
                        crate::config::modify(|c| c.accent_color = [r, g, b]);
                    }
                });

                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new("Profiles").strong());
                    let active_profile = crate::config::get_active_profile_name();
                    let profiles = crate::config::get_profiles();
                    
                    ui.horizontal(|ui| {
                        ui.label("Active Profile:");
                        egui::ComboBox::from_id_salt("profile_selector")
                            .selected_text(&active_profile)
                            .show_ui(ui, |ui| {
                                for p in profiles {
                                    if ui.selectable_label(p == active_profile, &p).clicked() {
                                        crate::config::set_active_profile(&p);
                                        self.reload_from_config();
                                        if let Err(e) = crate::lua_engine::apply_config() {
                                            log::warn!("apply_config after profile switch: {}", e);
                                        }
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_profile_buf).hint_text("New Profile Name..."));
                        if ui.button("Create").clicked() {
                            let name = self.new_profile_buf.trim().to_owned();
                            if !name.is_empty() {
                                crate::config::set_active_profile(&name);
                                self.reload_from_config();
                                if let Err(e) = crate::lua_engine::apply_config() {
                                    log::warn!("apply_config after profile create: {}", e);
                                }
                                self.new_profile_buf.clear();
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new("Zulip Configuration").strong());
                    egui::Grid::new("zulip_grid")
                        .num_columns(2)
                        .spacing([10.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("URL:");
                            ui.text_edit_singleline(&mut self.zulip_url_buf);
                            ui.end_row();

                            ui.label("Email:");
                            ui.text_edit_singleline(&mut self.zulip_email_buf);
                            ui.end_row();

                            ui.label("API Key:");
                            ui.text_edit_singleline(&mut self.zulip_key_buf);
                            ui.end_row();

                            ui.label("Stream:");
                            ui.text_edit_singleline(&mut self.zulip_stream_buf);
                            ui.end_row();

                            ui.label("Topic:");
                            ui.text_edit_singleline(&mut self.zulip_topic_buf);
                            ui.end_row();
                        });

                    ui.add_space(4.0);
                    if ui.button("Apply Zulip Settings").clicked() {
                        let mut cfg = crate::zulip::get_config();
                        cfg.url = self.zulip_url_buf.trim().to_owned();
                        cfg.email = self.zulip_email_buf.trim().to_owned();
                        cfg.api_key = self.zulip_key_buf.trim().to_owned();
                        cfg.stream = self.zulip_stream_buf.trim().to_owned();
                        cfg.topic = self.zulip_topic_buf.trim().to_owned();
                        crate::zulip::set_config(cfg);
                    }
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
        // Upload any decoded images from background threads to GPU textures.
        for (url, decoded) in crate::zulip::take_decoded_images() {
            let color_img = egui::ColorImage::from_rgba_unmultiplied(
                [decoded.width, decoded.height],
                &decoded.pixels,
            );
            let handle = ctx.load_texture(&url, color_img, Default::default());
            self.zulip_texture_cache.insert(url, handle);
        }

        let cfg = crate::zulip::get_config();

        // Trigger channel load if enabled and not yet loaded.
        if cfg.enabled && !cfg.url.is_empty() {
            crate::zulip::fetch_channels();
        }

        let channels = crate::zulip::get_channels();
        let channels_loaded = crate::zulip::channels_loaded();
        let sel_stream = self.zulip_selected_stream.clone();
        let sel_topic = self.zulip_selected_topic.clone();

        // Get messages for current selection.
        let messages: Vec<crate::zulip::ZulipMessage> = if !sel_stream.is_empty() && !sel_topic.is_empty() {
            crate::zulip::fetch_channel_messages(sel_stream.clone(), sel_topic.clone());
            crate::zulip::get_channel_messages(&sel_stream, &sel_topic).unwrap_or_default()
        } else if !sel_stream.is_empty() {
            crate::zulip::get_stream_messages(&sel_stream)
        } else {
            vec![]
        };
        let is_loading = !sel_topic.is_empty() && crate::zulip::is_channel_loading(&sel_stream, &sel_topic);

        // Kick off image fetches for visible messages.
        for msg in &messages {
            for url in &msg.image_urls {
                crate::zulip::fetch_image(url.clone());
            }
        }

        // Collect UI events into locals; apply after closure.
        let mut new_stream: Option<String> = None;
        let mut new_topic: Option<String> = None;
        let mut new_expanded: Option<Option<String>> = None;
        let mut fetch_topics_sid: Option<u64> = None;
        let mut do_toggle_settings = false;
        let mut do_apply_settings = false;
        let mut do_reset_channels = false;
        let mut send_msg: Option<(String, String, String)> = None;
        let mut set_enabled: Option<bool> = None;
        let mut open = self.zulip_open;

        egui::Window::new("Zulip")
            .id(egui::Id::new("anemoia_zulip"))
            .open(&mut open)
            .min_size([720.0, 460.0])
            .resizable(true)
            .show(ctx, |ui| {
                // ── Top bar ──────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    if cfg.enabled {
                        ui.colored_label(egui::Color32::from_rgb(100, 220, 100), "● Live");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "○ Offline");
                    }
                    ui.separator();
                    let settings_label = if self.zulip_in_settings { "✕ Settings" } else { "⚙ Settings" };
                    if ui.small_button(settings_label).clicked() {
                        do_toggle_settings = true;
                    }
                    if !self.zulip_in_settings {
                        if ui.small_button("↺").on_hover_text("Refresh channels").clicked() {
                            do_reset_channels = true;
                        }
                    }
                });
                ui.separator();

                // ── Settings panel ───────────────────────────────────────────
                if self.zulip_in_settings {
                    egui::Grid::new("zulip_cfg")
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Server URL:");
                            ui.add(egui::TextEdit::singleline(&mut self.zulip_url_buf).desired_width(280.0));
                            ui.end_row();
                            ui.label("Email:");
                            ui.add(egui::TextEdit::singleline(&mut self.zulip_email_buf).desired_width(280.0));
                            ui.end_row();
                            ui.label("API Key:");
                            ui.add(egui::TextEdit::singleline(&mut self.zulip_key_buf).password(true).desired_width(280.0));
                            ui.end_row();
                            ui.label("Default Stream:");
                            ui.add(egui::TextEdit::singleline(&mut self.zulip_stream_buf).desired_width(280.0));
                            ui.end_row();
                            ui.label("Default Topic:");
                            ui.add(egui::TextEdit::singleline(&mut self.zulip_topic_buf).desired_width(280.0));
                            ui.end_row();
                        });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() { do_apply_settings = true; }
                        if ui.button("Cancel").clicked() { do_toggle_settings = true; }
                        ui.separator();
                        if cfg.enabled {
                            if ui.button("Disable Bridge").clicked() { set_enabled = Some(false); }
                        } else {
                            if ui.button("Enable Bridge").clicked() { set_enabled = Some(true); }
                        }
                    });
                    return;
                }

                // ── Main layout: sidebar + message area ───────────────────────
                ui.horizontal_top(|ui| {
                    // Left: channel list
                    ui.vertical(|ui| {
                        ui.set_width(185.0);
                        if !channels_loaded {
                            if crate::zulip::channels_loading() {
                                ui.horizontal(|ui| { ui.spinner(); ui.label("Loading..."); });
                            } else {
                                ui.label(
                                    egui::RichText::new("Configure server\ncredentials in ⚙ Settings")
                                        .weak()
                                        .small(),
                                );
                            }
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("zulip_ch_list")
                                .max_height(370.0)
                                .show(ui, |ui| {
                                    for ch in &channels {
                                        let is_exp = self.zulip_expanded_stream.as_deref() == Some(ch.name.as_str());
                                        let stream_active = sel_stream == ch.name;

                                        ui.horizontal(|ui| {
                                            let icon = if is_exp { "▼" } else { "▶" };
                                            if ui.small_button(icon).clicked() {
                                                if is_exp {
                                                    new_expanded = Some(None);
                                                } else {
                                                    new_expanded = Some(Some(ch.name.clone()));
                                                    fetch_topics_sid = Some(ch.stream_id);
                                                }
                                            }
                                            let col = if stream_active && sel_topic.is_empty() {
                                                egui::Color32::from_rgb(120, 190, 255)
                                            } else {
                                                egui::Color32::GRAY
                                            };
                                            let lbl = egui::RichText::new(format!("#{}", ch.name)).small().color(col);
                                            if ui.selectable_label(stream_active && sel_topic.is_empty(), lbl).clicked() {
                                                new_stream = Some(ch.name.clone());
                                                new_topic = Some(String::new());
                                            }
                                        });

                                        if is_exp {
                                            if !ch.topics_loaded {
                                                ui.horizontal(|ui| {
                                                    ui.add_space(18.0);
                                                    ui.spinner();
                                                });
                                            } else if ch.topics.is_empty() {
                                                ui.horizontal(|ui| {
                                                    ui.add_space(18.0);
                                                    ui.label(egui::RichText::new("(no topics)").small().weak());
                                                });
                                            } else {
                                                for topic in &ch.topics {
                                                    let selected = sel_stream == ch.name && sel_topic == *topic;
                                                    ui.horizontal(|ui| {
                                                        ui.add_space(18.0);
                                                        let lbl = egui::RichText::new(topic).small();
                                                        if ui.selectable_label(selected, lbl).clicked() {
                                                            new_stream = Some(ch.name.clone());
                                                            new_topic = Some(topic.clone());
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                });
                        }
                    });

                    ui.separator();

                    // Right: message area
                    ui.vertical(|ui| {
                        if sel_stream.is_empty() {
                            ui.add_space(60.0);
                            ui.label(egui::RichText::new("← Select a channel").weak());
                            return;
                        }

                        // Header
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("#{}", sel_stream)).strong());
                            if !sel_topic.is_empty() {
                                ui.label(egui::RichText::new(format!(" › {}", sel_topic)).weak());
                            }
                        });
                        ui.separator();

                        if is_loading {
                            ui.horizontal(|ui| { ui.spinner(); ui.label("Loading history..."); });
                        }

                        // Determine how much height to give the scroll area.
                        let compose_height = if sel_topic.is_empty() { 50.0 } else { 28.0 };
                        let msg_height = (ui.available_height() - compose_height).max(80.0);

                        egui::ScrollArea::vertical()
                            .id_salt("zulip_msgs")
                            .max_height(msg_height)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for msg in &messages {
                                    // Sender line
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("[{}]", msg.time))
                                                .small()
                                                .weak(),
                                        );
                                        ui.label(
                                            egui::RichText::new(&msg.sender)
                                                .small()
                                                .strong()
                                                .color(egui::Color32::from_rgb(120, 200, 120)),
                                        );
                                    });

                                    // Content (strip markdown image syntax)
                                    let text = zulip_strip_md(&msg.content);
                                    let display = if text.len() > 600 {
                                        format!("{}…", &text[..600])
                                    } else {
                                        text
                                    };
                                    ui.label(egui::RichText::new(&display).small());

                                    // Inline images
                                    for img_url in &msg.image_urls {
                                        if let Some(handle) = self.zulip_texture_cache.get(img_url) {
                                            let tex_sz = handle.size_vec2();
                                            let max_px = 128.0_f32;
                                            let scale = (max_px / tex_sz.x.max(tex_sz.y)).min(1.0);
                                            let draw_sz = tex_sz * scale;
                                            let (rect, _) = ui.allocate_exact_size(draw_sz, egui::Sense::hover());
                                            ui.painter().image(
                                                handle.id(),
                                                rect,
                                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                egui::Color32::WHITE,
                                            );
                                        } else {
                                            match crate::zulip::get_image(img_url) {
                                                Some(crate::zulip::CachedImage::Loading) => { ui.spinner(); }
                                                Some(crate::zulip::CachedImage::Failed) => {
                                                    ui.label(egui::RichText::new("[img]").weak().small());
                                                }
                                                _ => { ui.spinner(); }
                                            }
                                        }
                                    }

                                    ui.add_space(2.0);
                                }
                            });

                        ui.separator();

                        // Topic input (only when viewing stream root, not a specific topic)
                        if sel_topic.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label("Topic:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.zulip_topic_buf)
                                        .desired_width(180.0)
                                        .hint_text("topic name"),
                                );
                            });
                        }

                        // Compose row
                        ui.horizontal(|ui| {
                            let avail = ui.available_width() - 58.0;
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.zulip_msg_buf)
                                    .desired_width(avail)
                                    .hint_text("Message…"),
                            );
                            let do_send = (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                || ui.button("Send").clicked();
                            if do_send {
                                let content = self.zulip_msg_buf.trim().to_owned();
                                let topic = if sel_topic.is_empty() {
                                    self.zulip_topic_buf.trim().to_owned()
                                } else {
                                    sel_topic.clone()
                                };
                                if !content.is_empty() && !topic.is_empty() {
                                    send_msg = Some((sel_stream.clone(), topic, content));
                                }
                            }
                        });
                    });
                });
            });

        self.zulip_open = open;

        // ── Apply collected mutations ──────────────────────────────────────────
        if let Some(s) = new_stream {
            self.zulip_selected_stream = s;
        }
        if let Some(t) = new_topic {
            self.zulip_selected_topic = t;
        }
        if let Some(exp) = new_expanded {
            self.zulip_expanded_stream = exp;
        }
        if let Some(sid) = fetch_topics_sid {
            crate::zulip::fetch_topics(sid);
        }
        if do_toggle_settings {
            self.zulip_in_settings = !self.zulip_in_settings;
            if self.zulip_in_settings {
                let c = crate::zulip::get_config();
                self.zulip_url_buf    = c.url;
                self.zulip_email_buf  = c.email;
                self.zulip_key_buf    = c.api_key;
                self.zulip_stream_buf = c.stream;
                self.zulip_topic_buf  = c.topic;
            }
        }
        if do_apply_settings {
            let mut c = crate::zulip::get_config();
            c.url     = self.zulip_url_buf.trim().to_owned();
            c.email   = self.zulip_email_buf.trim().to_owned();
            c.api_key = self.zulip_key_buf.trim().to_owned();
            c.stream  = self.zulip_stream_buf.trim().to_owned();
            c.topic   = self.zulip_topic_buf.trim().to_owned();
            crate::zulip::set_config(c);
            crate::zulip::reset_channels();
            crate::zulip::fetch_channels();
            self.zulip_in_settings = false;
        }
        if do_reset_channels {
            crate::zulip::reset_channels();
            crate::zulip::fetch_channels();
        }
        if let Some((stream, topic, content)) = send_msg {
            crate::zulip::send_to(stream, topic, content);
            self.zulip_msg_buf.clear();
        }
        if let Some(enabled) = set_enabled {
            let mut c = crate::zulip::get_config();
            c.enabled = enabled;
            crate::zulip::set_config(c);
        }
    }

    // ── Packet Manager ────────────────────────────────────────────────────────

    fn draw_packet_mgr(&mut self, ctx: &Context) {
        use crate::packet_capture::{self, Direction};

        let cap_arc = packet_capture::get();

        struct PktItem {
            id: u64,
            elapsed: f64,
            dir: Direction,
            short: String,
            cancelled: bool,
        }

        // Collect state under a single lock, then release before drawing.
        let (mut enabled, mut paused, mut show_out, mut show_in, mut search, items, selected_id, sel_type, sel_dir, sel_elapsed, sel_cancelled, sel_fields) = {
            let cap = cap_arc.lock().unwrap();
            let ids = cap.visible_ids();
            let items: Vec<PktItem> = ids.iter().filter_map(|&id| {
                cap.get(id).map(|p| PktItem {
                    id,
                    elapsed: p.elapsed,
                    dir: p.direction,
                    short: packet_capture::short_name(&p.type_name),
                    cancelled: p.cancelled,
                })
            }).collect();
            let sel = cap.selected_id;
            let sel_type  = sel.and_then(|id| cap.get(id)).map(|p| p.type_name.clone());
            let sel_dir   = sel.and_then(|id| cap.get(id)).map(|p| p.direction);
            let sel_el    = sel.and_then(|id| cap.get(id)).map(|p| p.elapsed);
            let sel_canc  = sel.and_then(|id| cap.get(id)).map(|p| p.cancelled).unwrap_or(false);
            let sel_fields = sel.and_then(|id| cap.get(id)).and_then(|p| p.fields.clone());
            (cap.enabled, cap.paused, cap.show_out, cap.show_in, cap.search.clone(),
             items, sel, sel_type, sel_dir, sel_el, sel_canc, sel_fields)
        };

        let mut do_clear = false;
        let mut clicked_id: Option<u64> = None;
        let mut open = self.packet_mgr_open;

        egui::Window::new("Packet Capture")
            .id(egui::Id::new("anemoia_packets"))
            .open(&mut open)
            .min_size([620.0, 400.0])
            .resizable(true)
            .show(ctx, |ui| {
                // Controls
                ui.horizontal(|ui| {
                    if ui.selectable_label(enabled, "Capture").clicked() { enabled = !enabled; }
                    if ui.selectable_label(paused, "Pause").clicked() { paused = !paused; }
                    if ui.button("Clear").clicked() { do_clear = true; }
                    ui.separator();
                    if ui.selectable_label(show_out, "OUT").clicked() { show_out = !show_out; }
                    if ui.selectable_label(show_in, "IN").clicked() { show_in = !show_in; }
                    ui.separator();
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut search);
                    ui.label(format!("{} pkts", items.len()));
                });

                ui.separator();

                ui.columns(2, |cols| {
                    // Left: packet list
                    egui::ScrollArea::vertical()
                        .id_salt("pkt_list")
                        .max_height(340.0)
                        .show(&mut cols[0], |ui| {
                            for item in &items {
                                let arrow = if item.dir == Direction::Out { "→" } else { "←" };
                                let canc  = if item.cancelled { " ✗" } else { "" };
                                let label = format!("{:.3}s {} {}{}", item.elapsed, arrow, item.short, canc);
                                let color = if item.dir == Direction::Out {
                                    egui::Color32::from_rgb(220, 200, 100)
                                } else {
                                    egui::Color32::from_rgb(100, 200, 220)
                                };
                                let rich = egui::RichText::new(&label).color(color).monospace().small();
                                if ui.selectable_label(selected_id == Some(item.id), rich).clicked() {
                                    clicked_id = Some(item.id);
                                }
                            }
                        });

                    // Right: field details
                    if let (Some(type_name), Some(dir), Some(elapsed)) = (&sel_type, sel_dir, sel_elapsed) {
                        cols[1].label(egui::RichText::new(packet_capture::short_name(type_name)).strong());
                        cols[1].label(egui::RichText::new(type_name.as_str()).small().weak());
                        cols[1].label(format!("{:?}  t={:.3}s", dir, elapsed));
                        if sel_cancelled {
                            cols[1].colored_label(egui::Color32::YELLOW, "CANCELLED (blocked by Lua)");
                        }
                        cols[1].separator();
                        match &sel_fields {
                            Some(fields) => {
                                egui::ScrollArea::vertical()
                                    .id_salt("pkt_fields")
                                    .max_height(300.0)
                                    .show(&mut cols[1], |ui| {
                                        for (fname, fval) in fields {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(egui::RichText::new(fname).strong().small());
                                                ui.label(egui::RichText::new(fval).small().weak());
                                            });
                                        }
                                    });
                            }
                            None => {
                                cols[1].label(egui::RichText::new("Reflecting…").weak());
                            }
                        }
                    } else {
                        cols[1].label(egui::RichText::new("Select a packet to inspect.").weak());
                    }
                });
            });

        // Apply control changes
        {
            let mut cap = cap_arc.lock().unwrap();
            cap.enabled  = enabled;
            cap.paused   = paused;
            cap.show_out = show_out;
            cap.show_in  = show_in;
            cap.search   = search;
            if do_clear { cap.clear(); }
            if let Some(id) = clicked_id {
                cap.selected_id = Some(id);
                // Clear cached fields so reflection is re-triggered
                if let Some(pkt) = cap.get_mut(id) {
                    if pkt.fields.is_none() {
                        // already None — reflection will happen next frame
                    }
                }
            }
        }

        self.packet_mgr_open = open;
    }

    fn maybe_reflect_selected(&mut self) {
        let cap_arc = crate::packet_capture::get();

        // Check if reflection is needed without holding the lock long.
        let (sel_id, raw) = {
            let cap = match cap_arc.try_lock() {
                Ok(c) => c,
                Err(_) => return,
            };
            let id = match cap.selected_id { Some(id) => id, None => return };
            let raw = match cap.get(id) {
                Some(p) if p.fields.is_none() => p.raw.as_obj().as_raw(),
                _ => return,
            };
            (id, raw)
        };

        let jvm = crate::jvm::Jvm::get();
        let mut env = match jvm.attach() {
            Ok(e) => e,
            Err(_) => return,
        };

        let fields = crate::mc::reflect::reflect_fields(&mut env, raw)
            .unwrap_or_else(|e| vec![("<error>".into(), e.to_string())]);

        let mut cap = match cap_arc.try_lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        if let Some(pkt) = cap.get_mut(sel_id) {
            pkt.fields = Some(fields);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn zulip_strip_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Skip markdown image syntax: ![alt](url)
        if i + 1 < len && bytes[i] == b'!' && bytes[i + 1] == b'[' {
            let mut depth = 0i32;
            let mut j = i + 2;
            while j < len {
                if bytes[j] == b'(' { depth += 1; }
                if bytes[j] == b')' {
                    if depth > 0 { depth -= 1; } else { j += 1; break; }
                }
                j += 1;
            }
            i = j;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

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
