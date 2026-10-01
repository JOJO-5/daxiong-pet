//! 极简的配置持久化：目前只记"上次选的是哪只宠物"。
//! 落在 Tauri 的 app_config_dir 下，随用户走。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// 上次选中的宠物 id
    #[serde(default)]
    pub pet_id: Option<String>,
    /// 重力开关：开启后宠物会往下掉，落到屏幕底部
    #[serde(default)]
    pub gravity: bool,
}

impl Config {
    fn path(app: &tauri::AppHandle) -> Option<PathBuf> {
        app.path()
            .app_config_dir()
            .ok()
            .map(|dir| dir.join("config.json"))
    }

    pub fn load(app: &tauri::AppHandle) -> Self {
        Self::path(app)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app: &tauri::AppHandle) {
        let Some(path) = Self::path(app) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}
