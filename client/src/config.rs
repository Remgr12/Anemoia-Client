use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use log::{warn, error};

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

    // category_name -> [x, y] screen position
    #[serde(default)]
    pub folder_positions: HashMap<String, [f32; 2]>,

    // RGB accent color [r, g, b] in 0-255
    #[serde(default = "default_accent_color")]
    pub accent_color: [u8; 3],
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
            folder_positions: Default::default(),
            accent_color: default_accent_color(),
        }
    }
}

fn default_toggle_key() -> i32 {
    crate::glfw::KEY_RIGHT_SHIFT // 344
}

fn default_accent_color() -> [u8; 3] {
    [180, 100, 255]
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

pub fn get_active_profile_name() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let path = PathBuf::from(home).join(".anemoia").join("active_profile.txt");
    match fs::read_to_string(&path) {
        Ok(s) => {
            let s = s.trim().to_string();
            if s.is_empty() { "default".to_string() } else { s }
        }
        Err(_) => "default".to_string(),
    }
}

pub fn set_active_profile(name: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let dir = PathBuf::from(home).join(".anemoia");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("active_profile.txt"), name);
    let container = CONFIG.get_or_init(|| Mutex::new(load_from_disk()));
    let mut lock = container.lock().unwrap();
    *lock = load_from_disk();
}

pub fn get_profiles() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let dir = PathBuf::from(home).join(".anemoia").join("profiles");
    let mut profiles = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            profiles.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    if !profiles.contains(&"default".to_string()) {
        profiles.push("default".to_string());
    }
    profiles.sort();
    profiles.dedup();
    profiles
}

fn migrate_old_config() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let old_path = PathBuf::from(&home).join(".anemoia").join("config.json");
    let new_dir = PathBuf::from(&home).join(".anemoia").join("profiles");
    let new_path = new_dir.join("default.json");
    if old_path.exists() && !new_path.exists() {
        let _ = fs::create_dir_all(&new_dir);
        let _ = fs::rename(old_path, new_path);
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let dir = PathBuf::from(home).join(".anemoia").join("profiles");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    let profile = get_active_profile_name();
    dir.join(format!("{}.json", profile))
}

fn load_from_disk() -> AppConfig {
    migrate_old_config();
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
