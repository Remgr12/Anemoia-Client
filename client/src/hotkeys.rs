//! Polls GLFW key states and toggles Lua modules.
//!
//! Each module can set `key = <GLFW_key_code>` in its table. This must be
//! polled from the main/render thread to respect GLFW thread-safety rules.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::{glfw::Glfw, lua_engine};

static PREV_KEYS: OnceLock<Mutex<HashMap<i32, bool>>> = OnceLock::new();

pub fn tick(glfw: &Glfw, window: *mut libc::c_void) {
    let prev_lock = PREV_KEYS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut prev = match prev_lock.try_lock() {
        Ok(guard) => guard,
        Err(_) => return, // Don't block render thread if contended
    };

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

pub fn stop() {
    if let Some(lock) = PREV_KEYS.get() {
        if let Ok(mut prev) = lock.try_lock() {
            prev.clear();
        }
    }
}
