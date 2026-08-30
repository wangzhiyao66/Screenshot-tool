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
    /// 窗口/控件智能识别：鼠标悬停自动勾勒窗口/控件边框
    #[serde(default = "default_true")]
    pub smart_detect: bool,
    /// 识别粒度：window = 仅窗口；control = 窗口 + 控件（需辅助功能权限）
    #[serde(default = "default_smart_level")]
    pub smart_detect_level: String,
}

fn default_true() -> bool {
    true
}
fn default_smart_level() -> String {
    "control".into()
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
            smart_detect: true,
            smart_detect_level: "control".into(),
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
