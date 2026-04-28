use anyhow::Result;
use jni::JNIEnv;
use log::{error, info, warn};
use mlua::prelude::*;
use parking_lot::Mutex;
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use crate::lua_api;

static LUA: OnceLock<Mutex<Lua>> = OnceLock::new();
// Tracks paths of scripts loaded at runtime (auto-loaded + GUI-added).
static SCRIPTS: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();

// ── Public data types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    /// GLFW key code for hotkey toggle, 0 = no binding.
    pub key: i32,
}

// ── Init / teardown ───────────────────────────────────────────────────────────

pub fn init() -> Result<()> {
    SCRIPTS.get_or_init(|| Arc::new(Mutex::new(Vec::new())));

    let lua = Lua::new();
    lua_api::register(&lua)?;
    load_scripts_from_dir(&lua)?;

    LUA.set(Mutex::new(lua))
        .map_err(|_| anyhow::anyhow!("Lua engine already initialized"))?;

    Ok(())
}

pub fn teardown() {
    // Lua state drops with the OnceLock when the library is unloaded.
}

// ── Per-frame dispatch ────────────────────────────────────────────────────────

/// Invoked every render frame from the glXSwapBuffers hook.
pub fn on_tick(env: &mut JNIEnv) -> Result<()> {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return Ok(()),
    };

    let result: LuaResult<()> = guard.scope(|_| {
        let anemoia: LuaTable = guard.globals().get("anemoia")?;
        let modules: LuaTable = anemoia.get("_modules")?;

        for i in 1..=modules.len()? {
            let module: LuaTable = match modules.get(i) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if !module.get::<bool>("enabled").unwrap_or(false) {
                continue;
            }

            let on_tick: LuaFunction = match module.get("on_tick") {
                Ok(f) => f,
                Err(_) => continue,
            };

            if let Err(e) = on_tick.call::<()>(module.clone()) {
                warn!("on_tick error: {}", e);
            }
        }

        Ok(())
    });

    result.map_err(|e| anyhow::anyhow!("Lua dispatch panic: {}", e))?;
    let _ = env;
    Ok(())
}

// ── Module list (GUI reads) ───────────────────────────────────────────────────

pub fn get_module_list() -> Vec<ModuleInfo> {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return vec![],
    };

    collect_modules(&guard).unwrap_or_default()
}

fn collect_modules(lua: &Lua) -> LuaResult<Vec<ModuleInfo>> {
    let anemoia: LuaTable = lua.globals().get("anemoia")?;
    let list: LuaTable = anemoia.get("_modules")?;
    let mut out = vec![];

    for i in 1..=list.len()? {
        let m: LuaTable = match list.get(i) {
            Ok(t) => t,
            Err(_) => continue,
        };
        out.push(ModuleInfo {
            name: m.get("name").unwrap_or_else(|_| format!("module_{}", i)),
            description: m.get("description").unwrap_or_default(),
            category: m.get("category").unwrap_or_else(|_| "Misc".into()),
            enabled: m.get("enabled").unwrap_or(false),
            key: m.get("key").unwrap_or(0),
        });
    }

    Ok(out)
}

// ── Module toggle (GUI writes) ────────────────────────────────────────────────

/// Flips a module's `enabled` flag and calls `on_enable` / `on_disable` if defined.
pub fn set_module_enabled(name: &str, enabled: bool) {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return,
    };

    let _ = (|| -> LuaResult<()> {
        let anemoia: LuaTable = guard.globals().get("anemoia")?;
        let list: LuaTable = anemoia.get("_modules")?;

        for i in 1..=list.len()? {
            let module: LuaTable = list.get(i)?;
            let module_name: String = module.get("name")?;
            if module_name != name {
                continue;
            }

            let was: bool = module.get("enabled").unwrap_or(false);
            if was == enabled {
                break;
            }

            module.set("enabled", enabled)?;

            let cb = if enabled { "on_enable" } else { "on_disable" };
            if let Ok(f) = module.get::<LuaFunction>(cb) {
                if let Err(e) = f.call::<()>(module.clone()) {
                    warn!("{} {} error: {}", name, cb, e);
                }
            }
            break;
        }
        Ok(())
    })();
}

// ── Script management (GUI reads/writes) ──────────────────────────────────────

pub fn get_loaded_scripts() -> Vec<String> {
    SCRIPTS
        .get()
        .map(|s| s.lock().clone())
        .unwrap_or_default()
}

/// Loads a single `.lua` file at runtime and tracks its path.
pub fn load_script_file(path: &str) -> Result<()> {
    let guard = LUA
        .get()
        .ok_or_else(|| anyhow::anyhow!("Lua engine not initialized"))?
        .lock();

    let src = std::fs::read_to_string(path)?;
    guard
        .load(&src)
        .set_name(format!("@{}", path))
        .exec()
        .map_err(|e| anyhow::anyhow!("Script error: {}", e))?;

    if let Some(scripts) = SCRIPTS.get() {
        let mut list = scripts.lock();
        if !list.iter().any(|p| p == path) {
            list.push(path.to_owned());
        }
    }

    info!("Loaded script: {}", path);
    Ok(())
}

/// Removes a script's path from the tracking list.
/// NOTE: Lua has no unload — modules registered by the script remain until full
/// restart. This removes it from the display and prevents future auto-loading.
pub fn forget_script(path: &str) {
    if let Some(scripts) = SCRIPTS.get() {
        scripts.lock().retain(|p| p != path);
    }
}

// ── Internal ──────────────────────────────────────────────────────────────────

fn scripts_dir() -> PathBuf {
    std::env::var("ANEMOIA_SCRIPTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config/anemoia/scripts")
        })
}

fn load_scripts_from_dir(lua: &Lua) -> Result<()> {
    let dir = scripts_dir();
    if !dir.exists() {
        info!("Scripts dir not found: {} — skipping", dir.display());
        return Ok(());
    }

    let scripts_tracker = SCRIPTS.get().unwrap();
    let mut count = 0usize;

    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lua") {
            continue;
        }

        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                error!("Cannot read {}: {}", path.display(), e);
                continue;
            }
        };

        match lua
            .load(&src)
            .set_name(format!("@{}", path.display()))
            .exec()
        {
            Ok(_) => {
                scripts_tracker
                    .lock()
                    .push(path.display().to_string());
                count += 1;
            }
            Err(e) => error!("Script error {}: {}", path.display(), e),
        }
    }

    info!("{} script(s) loaded", count);
    Ok(())
}
