//! Background thread that polls GLFW key states and toggles Lua modules.
//!
//! Each module can set `key = <GLFW_key_code>` in its table.  The thread
//! detects rising edges (key-down transitions) and calls
//! `lua_engine::set_module_enabled`.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use crate::{glfw::Glfw, lua_engine};

static HOTKEY_RUNNING: AtomicBool = AtomicBool::new(false);

/// Starts the hotkey polling thread.
///
/// `window_ptr` is passed as `usize` to satisfy `Send` — the underlying
/// GLFWwindow lives for the entire process lifetime.
pub fn start(glfw: Arc<Glfw>, window_ptr: usize) {
    if HOTKEY_RUNNING.swap(true, Ordering::SeqCst) {
        return; // already running
    }

    thread::spawn(move || {
        let window = window_ptr as *mut libc::c_void;
        let mut prev: HashMap<i32, bool> = HashMap::new();

        while HOTKEY_RUNNING.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));

            let modules = lua_engine::get_module_list();

            for m in modules {
                if m.key == 0 {
                    continue;
                }

                let down = glfw.key_pressed(window, m.key);
                let was_down = *prev.entry(m.key).or_insert(false);
                prev.insert(m.key, down);

                // Rising edge — toggle module.
                if down && !was_down {
                    let new_state = !m.enabled;
                    lua_engine::set_module_enabled(&m.name, new_state);
                }
            }
        }
    });
}

pub fn stop() {
    HOTKEY_RUNNING.store(false, Ordering::SeqCst);
}
