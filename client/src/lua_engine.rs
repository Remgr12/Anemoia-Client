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
static RENDER_CALLBACKS: OnceLock<Arc<Mutex<Vec<LuaRegistryKey>>>> = OnceLock::new();
static PACKET_SEND_CALLBACKS: OnceLock<Arc<Mutex<Vec<LuaRegistryKey>>>> = OnceLock::new();
static ZULIP_UI_CALLBACK: OnceLock<Arc<Mutex<Option<LuaRegistryKey>>>> = OnceLock::new();

// ── Public data types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    /// GLFW key code for hotkey toggle, 0 = no binding.
    pub key: i32,
    pub settings: Vec<ModuleSetting>,
}

#[derive(Debug, Clone)]
pub enum ModuleSetting {
    Boolean {
        name: String,
        value: bool,
    },
    Number {
        name: String,
        value: f64,
        min: f64,
        max: f64,
    },
    Keybind {
        name: String,
        value: i32,
    },
    Enum {
        name: String,
        value: String,
        options: Vec<String>,
    },
}

// ── Init / teardown ───────────────────────────────────────────────────────────

pub fn init() -> Result<()> {
    SCRIPTS.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    RENDER_CALLBACKS.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    PACKET_SEND_CALLBACKS.get_or_init(|| Arc::new(Mutex::new(Vec::new())));

    let lua = Lua::new();
    
    // Set package.path to include the scripts directory and its parent
    {
        let scripts_path = scripts_dir();
        let parent_path = scripts_path.parent().unwrap_or(&scripts_path);
        let s_str = scripts_path.to_string_lossy();
        let p_str = parent_path.to_string_lossy();
        
        let lua_path = format!("{};{}/?.lua;{}/?/init.lua;{}/?.lua;{}/?/init.lua", 
            lua.globals().get::<LuaTable>("package")?.get::<String>("path")?,
            s_str, s_str, p_str, p_str);
        lua.globals().get::<LuaTable>("package")?.set("path", lua_path)?;
    }

    lua_api::register(&lua)?;
    load_scripts_from_dir(&lua)?;

    LUA.set(Mutex::new(lua))
        .map_err(|_| anyhow::anyhow!("Lua engine already initialized"))?;

    // Load any extra scripts from config
    let extra_scripts = crate::config::get().loaded_scripts;
    for script in extra_scripts {
        if let Err(e) = load_script_file(&script) {
            warn!("Failed to load persisted script {}: {}", script, e);
        }
    }

    // Apply config settings
    if let Err(e) = apply_config() {
        warn!("Failed to apply config to Lua state: {}", e);
    }

    Ok(())
}

fn apply_config() -> Result<()> {
    let guard = LUA.get().unwrap().lock();
    let cfg = crate::config::get();
    
    let anemoia: LuaTable = guard.globals().get("anemoia")?;
    let list: LuaTable = anemoia.get("_modules")?;

    for i in 1..=list.len()? {
        let module: LuaTable = list.get(i)?;
        let name: String = module.get("name")?;

        if let Some(&enabled) = cfg.module_enabled.get(&name) {
            let was: bool = module.get("enabled").unwrap_or(false);
            if was != enabled {
                module.set("enabled", enabled)?;
                let cb = if enabled { "on_enable" } else { "on_disable" };
                if let Ok(f) = module.get::<LuaFunction>(cb) {
                    if let Err(e) = f.call::<()>(module.clone()) {
                        warn!("{} {} error: {}", name, cb, e);
                    }
                }
            }
        }

        if let Some(&key) = cfg.module_keys.get(&name) {
            module.set("key", key)?;
        }

        if let Some(settings_map) = cfg.module_settings.get(&name) {
            if let Ok(settings) = module.get::<LuaTable>("settings") {
                for (k, v) in settings_map {
                    match v {
                        serde_json::Value::Bool(b) => { settings.set(k.as_str(), *b)?; }
                        serde_json::Value::Number(n) => { 
                            if let Some(f) = n.as_f64() {
                                settings.set(k.as_str(), f)?;
                            }
                        }
                        serde_json::Value::String(s) => { settings.set(k.as_str(), s.as_str())?; }
                        _ => {}
                    }
                }
            }
        }
    }
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

pub fn get_render_callbacks() -> Arc<Mutex<Vec<LuaRegistryKey>>> {
    RENDER_CALLBACKS.get().unwrap().clone()
}

pub fn get_packet_send_callbacks() -> Arc<Mutex<Vec<LuaRegistryKey>>> {
    PACKET_SEND_CALLBACKS.get().unwrap().clone()
}

pub fn get_zulip_ui_callback() -> Arc<Mutex<Option<LuaRegistryKey>>> {
    ZULIP_UI_CALLBACK.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

pub fn on_packet_send(packet: crate::mc::packet::Packet) -> Result<bool> {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return Ok(false),
    };

    let callbacks_mutex = PACKET_SEND_CALLBACKS.get().unwrap();
    let callbacks = callbacks_mutex.lock();
    if callbacks.is_empty() {
        return Ok(false);
    }

    let result: LuaResult<bool> = guard.scope(|scope| {
        let ud = scope.create_any_userdata(packet, |_| {})?;
        let mut cancelled = false;
        for key in callbacks.iter() {
            let func: LuaFunction = guard.registry_value(key)?;
            // If any callback returns true, packet is cancelled
            if let Ok(res) = func.call::<bool>(ud.clone()) {
                if res {
                    cancelled = true;
                }
            }
        }
        Ok(cancelled)
    });

    result.map_err(|e| anyhow::anyhow!("Lua packet error: {}", e))
}

pub fn on_render(painter: egui::Painter) -> Result<()> {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return Ok(()),
    };

    let callbacks_mutex = RENDER_CALLBACKS.get().unwrap();
    let callbacks = callbacks_mutex.lock();
    if callbacks.is_empty() {
        return Ok(());
    }

    let lua_painter = lua_api::render::LuaPainter { painter };
    let ud = guard.create_userdata(lua_painter)?;

    for key in callbacks.iter() {
        let func: LuaFunction = guard.registry_value(key)?;
        if let Err(e) = func.call::<()>(ud.clone()) {
            warn!("on_render error: {}", e);
        }
    }
    Ok(())
}

pub fn on_zulip_ui(painter: egui::Painter) -> Result<()> {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return Ok(()),
    };

    let cb_mutex = ZULIP_UI_CALLBACK.get_or_init(|| Arc::new(Mutex::new(None)));
    let cb_lock = cb_mutex.lock();
    if let Some(key) = cb_lock.as_ref() {
        let lua_painter = lua_api::render::LuaPainter { painter };
        let ud = guard.create_userdata(lua_painter)?;
        let func: LuaFunction = guard.registry_value(key)?;
        if let Err(e) = func.call::<()>(ud) {
            warn!("on_zulip_ui error: {}", e);
        }
    }
    Ok(())
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

        let mut settings = vec![];
        if let Ok(st) = m.get::<LuaTable>("settings") {
            for pair in st.pairs::<String, LuaValue>() {
                if let Ok((name, val)) = pair {
                    match val {
                        LuaValue::Boolean(b) => settings.push(ModuleSetting::Boolean { name, value: b }),
                        LuaValue::Number(n) => {
                            let mut min = 0.0;
                            let mut max = 100.0;
                            let mut is_keybind = false;
                            if let Ok(meta) = m.get::<LuaTable>("_settings_meta") {
                                if let Ok(m_table) = meta.get::<LuaTable>(name.clone()) {
                                    if let Ok(t) = m_table.get::<String>("type") {
                                        if t == "keybind" {
                                            is_keybind = true;
                                        }
                                    }
                                    min = m_table.get("min").unwrap_or(0.0);
                                    max = m_table.get("max").unwrap_or(100.0);
                                }
                            }
                            if is_keybind {
                                settings.push(ModuleSetting::Keybind { name, value: n as i32 });
                            } else {
                                settings.push(ModuleSetting::Number { name, value: n, min, max });
                            }
                        }
                        LuaValue::String(s) => {
                            let val_str = s.to_str().map(|b| b.to_string()).unwrap_or_default();
                            let mut options = vec![];
                            let mut is_enum = false;
                            if let Ok(meta) = m.get::<LuaTable>("_settings_meta") {
                                if let Ok(m_table) = meta.get::<LuaTable>(name.clone()) {
                                    if let Ok(t) = m_table.get::<String>("type") {
                                        if t == "enum" {
                                            is_enum = true;
                                            if let Ok(opts) = m_table.get::<LuaTable>("options") {
                                                for j in 1..=opts.len()? {
                                                    if let Ok(opt) = opts.get::<String>(j) {
                                                        options.push(opt);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if is_enum {
                                settings.push(ModuleSetting::Enum { name, value: val_str, options });
                            }
                        }
                        LuaValue::Integer(i) => {
                             settings.push(ModuleSetting::Keybind { name, value: i as i32 });
                        }
                        _ => {}
                    }
                }
            }
        }

        out.push(ModuleInfo {
            name: m.get("name").unwrap_or_else(|_| format!("module_{}", i)),
            description: m.get("description").unwrap_or_default(),
            category: m.get("category").unwrap_or_else(|_| "Misc".into()),
            enabled: m.get("enabled").unwrap_or(false),
            key: m.get("key").unwrap_or(0),
            settings,
        });
    }

    Ok(out)
}

// ── Module toggle (GUI writes) ────────────────────────────────────────────────

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

pub fn set_module_setting(module_name: &str, setting_name: &str, value: LuaValue) {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return,
    };

    let _ = (|| -> LuaResult<()> {
        let anemoia: LuaTable = guard.globals().get("anemoia")?;
        let list: LuaTable = anemoia.get("_modules")?;

        for i in 1..=list.len()? {
            let module: LuaTable = list.get(i)?;
            let name: String = module.get("name")?;
            if name != module_name {
                continue;
            }

            let settings: LuaTable = module.get("settings")?;
            settings.set(setting_name, value)?;
            break;
        }
        Ok(())
    })();
}

pub fn set_module_key(module_name: &str, key: i32) {
    let guard = match LUA.get() {
        Some(m) => m.lock(),
        None => return,
    };

    let _ = (|| -> LuaResult<()> {
        let anemoia: LuaTable = guard.globals().get("anemoia")?;
        let list: LuaTable = anemoia.get("_modules")?;

        for i in 1..=list.len()? {
            let module: LuaTable = list.get(i)?;
            let name: String = module.get("name")?;
            if name != module_name {
                continue;
            }

            module.set("key", key)?;
            crate::config::modify(|c| { c.module_keys.insert(module_name.to_string(), key); });
            break;
        }
        Ok(())
    })();
}

pub fn create_string(s: &str) -> mlua::Value {
    let guard = LUA.get().unwrap().lock();
    let s_obj = guard.create_string(s).unwrap();
    mlua::Value::String(unsafe { std::mem::transmute(s_obj) })
}

// ── Script management (GUI reads/writes) ──────────────────────────────────────

pub fn get_loaded_scripts() -> Vec<String> {
    SCRIPTS
        .get()
        .map(|s| s.lock().clone())
        .unwrap_or_default()
}

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

pub fn forget_script(path: &str) {
    if let Some(scripts) = SCRIPTS.get() {
        scripts.lock().retain(|p| p != path);
    }
}

// ── Internal ──────────────────────────────────────────────────────────────────

fn scripts_dir() -> PathBuf {
    // 1. Explicit override
    if let Ok(p) = std::env::var("ANEMOIA_SCRIPTS") {
        return PathBuf::from(p);
    }
    // 2. Set by agent_loader from the original .so path before dlopen
    if let Ok(root) = std::env::var("ANEMOIA_ROOT") {
        return PathBuf::from(root).join("scripts");
    }
    // 3. Compile-time fallback for direct/dev builds
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts"))
}

fn load_scripts_from_dir(lua: &Lua) -> Result<()> {
    let dir = scripts_dir();
    if !dir.exists() {
        info!("Scripts dir not found: {} — skipping", dir.display());
        return Ok(());
    }

    let scripts_tracker = SCRIPTS.get().unwrap();
    let mut count = 0usize;
    let mut stack = vec![dir];

    while let Some(current_dir) = stack.pop() {
        for entry in std::fs::read_dir(&current_dir)?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

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
    }

    info!("{} script(s) loaded", count);
    Ok(())
}
