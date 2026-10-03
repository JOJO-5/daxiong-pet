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
    #[serde(default)]
    pub shortcut_enabled: bool,
}

impl Config {
    fn path(app: &tauri::AppHandle) -> Option<PathBuf> {
        app.path()
            .app_config_dir()
            .ok()
            .map(|dir| dir.join("config.json"))
    }

    pub fn load(app: &tauri::AppHandle) -> std::io::Result<Self> {
        let path = Self::path(app).ok_or_else(|| std::io::Error::other("无法定位配置目录"))?;
        Self::load_path(&path)
    }

    fn load_path(path: &std::path::Path) -> std::io::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(std::io::Error::other),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, app: &tauri::AppHandle) -> std::io::Result<()> {
        let path = Self::path(app).ok_or_else(|| std::io::Error::other("无法定位配置目录"))?;
        self.save_path(&path)
    }

    fn save_path(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let dir = path.parent().ok_or_else(|| std::io::Error::other("配置目录无效"))?;
        std::fs::create_dir_all(dir)?;
        let mut temporary = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut temporary, self).map_err(std::io::Error::other)?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_replacement_is_readable_and_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        Config::default().save_path(&path).unwrap();
        Config { pet_id: Some("daxiong".into()), gravity: true, shortcut_enabled:true }.save_path(&path).unwrap();
        let loaded = Config::load_path(&path).unwrap();
        assert!(loaded.gravity);assert!(loaded.shortcut_enabled);
        assert_eq!(loaded.pet_id.as_deref(), Some("daxiong"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn corrupt_config_and_write_failures_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        assert!(!Config::load_path(&path).unwrap().gravity);
        std::fs::write(&path, b"broken").unwrap();
        assert!(Config::load_path(&path).is_err());
        assert!(Config::default().save_path(&path.join("config.json")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    }
}
