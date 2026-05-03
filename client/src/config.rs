use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use log::{info, warn, error};

use crate::zulip::ZulipConfig;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_toggle_key")]
    pub gui_toggle_key: i32,
    #[serde(default)]
    pub zulip: ZulipConfig,
    #[serde(default)]
    pub loaded_scripts: Vec<String>,
    
    // module_name -> setting_name -> setting_value
    #[serde(default)]
    pub module_settings: HashMap<String, HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub module_enabled: HashMap<String, bool>,
    #[serde(default)]
    pub module_keys: HashMap<String, i32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gui_toggle_key: default_toggle_key(),
            zulip: Default::default(),
            loaded_scripts: Default::default(),
            module_settings: Default::default(),
            module_enabled: Default::default(),
            module_keys: Default::default(),
        }
    }
}

fn default_toggle_key() -> i32 {
    crate::glfw::KEY_RIGHT_SHIFT // 344
}

static CONFIG: OnceLock<Mutex<AppConfig>> = OnceLock::new();

pub fn get() -> AppConfig {
    CONFIG.get_or_init(|| Mutex::new(load_from_disk())).lock().unwrap().clone()
}

pub fn modify<F>(f: F)
where
    F: FnOnce(&mut AppConfig),
{
    let container = CONFIG.get_or_init(|| Mutex::new(load_from_disk()));
    let mut lock = container.lock().unwrap();
    f(&mut lock);
    save_to_disk(&lock);
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let dir = PathBuf::from(home).join(".anemoia");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir.join("config.json")
}

fn load_from_disk() -> AppConfig {
    let path = config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to parse config: {}. Using default.", e);
                AppConfig::default()
            }
        },
        Err(e) => {
            warn!("Failed to read config: {}. Using default.", e);
            AppConfig::default()
        }
    }
}

fn save_to_disk(cfg: &AppConfig) {
    let path = config_path();
    match serde_json::to_string_pretty(cfg) {
        Ok(s) => {
            if let Err(e) = fs::write(&path, s) {
                error!("Failed to write config: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to serialize config: {}", e);
        }
    }
}
