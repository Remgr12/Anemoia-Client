//! Thin wrapper around GLFW functions resolved at runtime via dlsym.
//!
//! GLFW is already loaded in the JVM process (LWJGL3 packs it as a native lib)
//! so all symbols are available through RTLD_DEFAULT.

use std::sync::Arc;

// ── Key codes ────────────────────────────────────────────────────────────────

pub const KEY_RIGHT_SHIFT: i32 = 344;
pub const KEY_RIGHT_CTRL: i32 = 345;
pub const KEY_RIGHT_ALT: i32 = 346;
pub const KEY_LEFT_SHIFT: i32 = 340;
pub const KEY_LEFT_CTRL: i32 = 341;
pub const KEY_LEFT_ALT: i32 = 342;
pub const KEY_F1: i32 = 290;
pub const KEY_F2: i32 = 291;
pub const KEY_F3: i32 = 292;
pub const KEY_F4: i32 = 293;
pub const KEY_F5: i32 = 294;
pub const KEY_F6: i32 = 295;
pub const KEY_F7: i32 = 296;
pub const KEY_F8: i32 = 297;
pub const KEY_F9: i32 = 298;
pub const KEY_F10: i32 = 299;
pub const KEY_F11: i32 = 300;
pub const KEY_F12: i32 = 301;

const GLFW_PRESS: i32 = 1;
const GLFW_MOUSE_BUTTON_LEFT: i32 = 0;
const GLFW_CURSOR: i32 = 0x0003_3001;
const GLFW_CURSOR_NORMAL: i32 = 0x0003_4001;

// ── Function pointer table ────────────────────────────────────────────────────

pub struct Glfw {
    get_key: unsafe extern "C" fn(*mut libc::c_void, i32) -> i32,
    get_mouse_btn: unsafe extern "C" fn(*mut libc::c_void, i32) -> i32,
    get_cursor_pos: unsafe extern "C" fn(*mut libc::c_void, *mut f64, *mut f64),
    set_input_mode: unsafe extern "C" fn(*mut libc::c_void, i32, i32),
}

// Safety: glfwGetKey / glfwGetMouseButton are documented as safe to call from
// any thread in GLFW 3.x.  We never call the windowing/event functions from
// non-main threads.
unsafe impl Send for Glfw {}
unsafe impl Sync for Glfw {}

impl Glfw {
    pub unsafe fn load() -> anyhow::Result<Arc<Self>> {
        macro_rules! sym {
            ($name:literal) => {{
                let ptr = libc::dlsym(
                    libc::RTLD_DEFAULT,
                    concat!($name, "\0").as_ptr() as *const libc::c_char,
                );
                anyhow::ensure!(!ptr.is_null(), "GLFW symbol not found: {}", $name);
                std::mem::transmute(ptr)
            }};
        }
        Ok(Arc::new(Glfw {
            get_key: sym!("glfwGetKey"),
            get_mouse_btn: sym!("glfwGetMouseButton"),
            get_cursor_pos: sym!("glfwGetCursorPos"),
            set_input_mode: sym!("glfwSetInputMode"),
        }))
    }

    pub fn key_pressed(&self, win: *mut libc::c_void, key: i32) -> bool {
        unsafe { (self.get_key)(win, key) == GLFW_PRESS }
    }

    pub fn mouse_left_down(&self, win: *mut libc::c_void) -> bool {
        unsafe { (self.get_mouse_btn)(win, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS }
    }

    pub fn cursor_pos(&self, win: *mut libc::c_void) -> (f64, f64) {
        let (mut x, mut y) = (0f64, 0f64);
        unsafe { (self.get_cursor_pos)(win, &mut x, &mut y) };
        (x, y)
    }

    pub fn show_cursor(&self, win: *mut libc::c_void) {
        unsafe { (self.set_input_mode)(win, GLFW_CURSOR, GLFW_CURSOR_NORMAL) };
    }

    /// Scans the common key range and returns the first key currently held.
    /// Used for interactive key-binding in the Settings window.
    pub fn scan_any_pressed(&self, win: *mut libc::c_void) -> Option<i32> {
        // Printable / symbol keys (32–96) + control keys (256–348).
        let ranges: [(i32, i32); 2] = [(32, 96), (256, 348)];
        for (start, end) in ranges {
            for key in start..=end {
                if self.key_pressed(win, key) {
                    return Some(key);
                }
            }
        }
        None
    }
}

// ── Human-readable key names ──────────────────────────────────────────────────

pub fn key_name(key: i32) -> String {
    match key {
        0 => "None".into(),
        32 => "Space".into(),
        256 => "Escape".into(),
        257 => "Enter".into(),
        258 => "Tab".into(),
        259 => "Backspace".into(),
        KEY_LEFT_SHIFT => "Left Shift".into(),
        KEY_LEFT_CTRL => "Left Ctrl".into(),
        KEY_LEFT_ALT => "Left Alt".into(),
        KEY_RIGHT_SHIFT => "Right Shift".into(),
        KEY_RIGHT_CTRL => "Right Ctrl".into(),
        KEY_RIGHT_ALT => "Right Alt".into(),
        KEY_F1 => "F1".into(),
        KEY_F2 => "F2".into(),
        KEY_F3 => "F3".into(),
        KEY_F4 => "F4".into(),
        KEY_F5 => "F5".into(),
        KEY_F6 => "F6".into(),
        KEY_F7 => "F7".into(),
        KEY_F8 => "F8".into(),
        KEY_F9 => "F9".into(),
        KEY_F10 => "F10".into(),
        KEY_F11 => "F11".into(),
        KEY_F12 => "F12".into(),
        k if (65..=90).contains(&k) => format!("{}", (k as u8) as char),
        k if (48..=57).contains(&k) => format!("{}", (k as u8) as char),
        k => format!("Key({})", k),
    }
}
