use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub capture_hotkey: String,
    pub fixed_hotkey: String,
    pub pin_hotkey: String,
    pub ocr_lang: String,
    pub translate_provider: String,
    pub translate_key: String,
    pub translate_endpoint: String,
    pub translate_model: String,
    pub save_dir: String,
    #[serde(default)]
    pub fixed_regions: HashMap<String, Region>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_hotkey: "CommandOrControl+Shift+A".into(),
            fixed_hotkey: "CommandOrControl+Shift+R".into(),
            pin_hotkey: "CommandOrControl+Shift+V".into(),
            ocr_lang: "zh-Hans,en-US".into(),
            translate_provider: "google".into(),
            translate_key: String::new(),
            translate_endpoint: String::new(),
            translate_model: "gpt-4o-mini".into(),
            save_dir: String::new(),
            fixed_regions: HashMap::new(),
        }
    }
}

pub fn settings_path(dir: &PathBuf) -> PathBuf {
    let _ = std::fs::create_dir_all(dir);
    dir.join("settings.json")
}

pub fn load(dir: &PathBuf) -> Settings {
    let p = settings_path(dir);
    match std::fs::read_to_string(p) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(dir: &PathBuf, s: &Settings) -> Result<(), String> {
    let p = settings_path(dir);
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| e.to_string())
}
