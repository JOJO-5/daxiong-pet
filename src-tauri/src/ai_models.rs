//! Fixed model catalog; optional downloads never replace bundled resources.
use crate::ai_store::Result;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
pub fn spec(id: &str) -> Result<Value> {
    let default: Value = serde_json::from_str(include_str!(
        "../../scripts/ai-prototype/default-model.json"
    ))
    .map_err(|_| "模型清单无效。")?;
    match id {
        "2b" => Ok(default["default_text_model"].clone()),
        "vision" => Ok(default["vision_component"].clone()),
        "small" => {
            let candidates: Value = serde_json::from_str(include_str!(
                "../../docs/research/chat-model-candidates-2026-10-10.json"
            ))
            .map_err(|_| "模型清单无效。")?;
            candidates["candidates"]
                .as_array()
                .and_then(|a| a.iter().find(|s| s["name"] == "Qwen3.5-0.8B-Q4_K_M.gguf"))
                .cloned()
                .ok_or("模型清单无效。".into())
        }
        _ => Err("未知模型。".into()),
    }
}
pub fn verified(path: &Path, spec: &Value, token: &AtomicBool) -> Result<()> {
    let mut file = std::fs::File::open(path).map_err(|_| "模型文件不存在。")?;
    if file.metadata().map_err(|_| "模型读取失败。")?.len() != spec["bytes"].as_u64().unwrap_or(0)
    {
        return Err("模型大小不符，请重试下载或修复。".into());
    }
    let mut sha = Sha256::new();
    let mut buf = vec![0; 1024 * 1024];
    loop {
        if token.load(Ordering::SeqCst) {
            return Err("模型操作已停止。".into());
        }
        let n = file.read(&mut buf).map_err(|_| "模型读取失败。")?;
        if n == 0 {
            break;
        }
        sha.update(&buf[..n]);
    }
    if format!("{:x}", sha.finalize()) != spec["sha256"].as_str().unwrap_or("") {
        return Err("模型校验失败，请重试下载或修复。".into());
    }
    Ok(())
}
pub fn resolve(cache: &Path, bundled: &Path, spec: &Value, token: &AtomicBool) -> Result<PathBuf> {
    let name = spec["name"].as_str().ok_or("模型清单无效。")?;
    let cached = cache.join(name);
    if verified(&cached, spec, token).is_ok() {
        return Ok(cached);
    }
    if token.load(Ordering::SeqCst) {
        return Err("模型操作已停止。".into());
    }
    let path = bundled.join("models").join(name);
    verified(&path, spec, token)?;
    Ok(path)
}
#[derive(Clone, Serialize)]
pub struct Download {
    pub id: Option<String>,
    pub received: u64,
    pub total: u64,
    pub status: String,
    pub busy: bool,
}
#[derive(Serialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub installed: bool,
    pub cached: bool,
    pub purpose: String,
}
pub struct ModelManager {
    pub root: PathBuf,
    bundled: PathBuf,
    state: Mutex<Download>,
    token: Mutex<Option<Arc<AtomicBool>>>,
}
impl ModelManager {
    pub fn new(root: PathBuf, bundled: PathBuf) -> Self {
        // Only fixed, inactive temporary filenames owned by this manager are reclaimed.
        for id in ["2b", "vision", "small"] {
            if let Ok(spec) = spec(id) {
                if let Some(name) = spec["name"].as_str() {
                    let _ = std::fs::remove_file(root.join(name).with_extension("part"));
                }
            }
        }
        Self {
            root,
            bundled,
            state: Mutex::new(Download {
                id: None,
                received: 0,
                total: 0,
                status: "没有正在下载的模型。".into(),
                busy: false,
            }),
            token: Mutex::new(None),
        }
    }
    pub fn view(&self) -> Download {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn catalog(&self, bundled: &Path) -> Vec<Item> {
        ["2b", "vision", "small"]
            .into_iter()
            .filter_map(|id| {
                let s = spec(id).ok()?;
                let name = s["name"].as_str()?.to_owned();
                let bytes = s["bytes"].as_u64()?;
                let cached = self
                    .root
                    .join(&name)
                    .metadata()
                    .is_ok_and(|m| m.len() == bytes);
                let installed = cached
                    || bundled
                        .join("models")
                        .join(&name)
                        .metadata()
                        .is_ok_and(|m| m.len() == bytes);
                Some(Item {
                    id: id.into(),
                    name,
                    bytes,
                    installed,
                    cached,
                    purpose: match id {
                        "small" => "可选轻量文字模型，低资源时使用；不含视觉，复杂回答能力较弱。",
                        "vision" => "2B匹配视觉组件，安装包已内置；下载可修复损坏的组件。",
                        _ => "默认2B文字模型，安装包已内置；下载可修复损坏的权重。",
                    }
                    .into(),
                })
            })
            .collect()
    }
    pub fn cancel(&self) {
        if let Some(token) = self
            .token
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            token.store(true, Ordering::SeqCst);
        }
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.busy {
            return Err("请先停止下载。".into());
        }
        let s = spec(id)?;
        let path = self.root.join(s["name"].as_str().ok_or("模型清单无效。")?);
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("删除模型失败，请检查文件权限。".into()),
        }
    }
    pub fn download(self: &Arc<Self>, id: String) -> Result<()> {
        let spec = spec(&id)?;
        let token = Arc::new(AtomicBool::new(false));
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.busy {
                return Err("已有模型正在下载。".into());
            }
            *state = Download {
                id: Some(id),
                received: 0,
                total: spec["bytes"].as_u64().unwrap_or(0),
                status: "正在校验已有文件或下载。".into(),
                busy: true,
            };
            *self.token.lock().unwrap_or_else(|p| p.into_inner()) = Some(token.clone());
        }
        let manager = self.clone();
        if std::thread::Builder::new()
            .name("pet-model-download".into())
            .spawn(move || {
                let result = manager.fetch(&spec, &token);
                let mut state = manager.state.lock().unwrap_or_else(|p| p.into_inner());
                state.busy = false;
                state.status = match result {
                    Ok(()) => "下载完成，大小与SHA-256校验通过。".into(),
                    Err(e) => e,
                };
                *manager.token.lock().unwrap_or_else(|p| p.into_inner()) = None;
            })
            .is_err()
        {
            self.state.lock().unwrap_or_else(|p| p.into_inner()).busy = false;
            return Err("无法启动下载任务。".into());
        }
        Ok(())
    }
    fn fetch(&self, spec: &Value, token: &AtomicBool) -> Result<()> {
        std::fs::create_dir_all(&self.root)
            .map_err(|_| "无法创建模型目录，请检查磁盘空间或权限。")?;
        let path = self
            .root
            .join(spec["name"].as_str().ok_or("模型清单无效。")?);
        if verified(&path, spec, token).is_ok()
            || verified(
                &self
                    .bundled
                    .join("models")
                    .join(spec["name"].as_str().ok_or("模型清单无效。")?),
                spec,
                token,
            )
            .is_ok()
        {
            return Ok(());
        }
        if token.load(Ordering::SeqCst) {
            return Err("下载已停止。".into());
        }
        let partial = path.with_extension("part");
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            spec["repository"].as_str().ok_or("模型清单无效。")?,
            spec["revision"].as_str().ok_or("模型清单无效。")?,
            spec["name"].as_str().ok_or("模型清单无效。")?
        );
        let result = tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(3600))
                .build()
                .map_err(|_| "无法创建模型下载连接。")?;
            let future = client.get(url).send();
            tokio::pin!(future);
            let response = loop {
                if token.load(Ordering::SeqCst) {
                    return Err("下载已停止。".into());
                }
                tokio::select! {value=&mut future=>break value.map_err(|_|"模型下载连接失败，可以重试。")?,_=tokio::time::sleep(Duration::from_millis(25))=>{}}
            };
            if !response.status().is_success() {
                return Err("模型来源暂不可用，可以重试。".into());
            }
            let mut stream = response.bytes_stream();
            let mut file =
                std::fs::File::create(&partial).map_err(|_| "磁盘空间不足或模型目录不可写。")?;
            let mut count = 0_u64;
            let total = spec["bytes"].as_u64().ok_or("模型清单无效。")?;
            let mut digest = Sha256::new();
            loop {
                if token.load(Ordering::SeqCst) {
                    return Err("下载已停止。".into());
                }
                let next = tokio::select! {chunk=stream.next()=>chunk,_=tokio::time::sleep(Duration::from_millis(25))=>continue};
                let Some(chunk) = next else {
                    break;
                };
                let chunk = chunk.map_err(|_| "模型下载中断，可以重试。")?;
                count += chunk.len() as u64;
                if count > total {
                    return Err("下载大小超过固定清单，已停止。".into());
                }
                file.write_all(&chunk)
                    .map_err(|_| "磁盘空间不足，下载已停止。")?;
                digest.update(&chunk);
                self.state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .received = count;
            }
            if count != total
                || format!("{:x}", digest.finalize()) != spec["sha256"].as_str().unwrap_or("")
            {
                return Err("下载校验失败，未激活模型，请重试。".into());
            }
            file.sync_all()
                .map_err(|_| "保存模型失败，请检查磁盘空间。")?;
            drop(file);
            if token.load(Ordering::SeqCst) {
                return Err("下载已停止。".into());
            }
            // Keep any existing verified copy if interrupted before the rename.
            if path.exists() {
                std::fs::remove_file(&path).map_err(|_| "无法替换损坏模型。")?;
            }
            std::fs::rename(&partial, &path).map_err(|_| "无法完成模型安装。".into())
        });
        if result.is_err() {
            let _ = std::fs::remove_file(partial);
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_or_old_cached_weight_falls_back_to_verified_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let bundled = dir.path().join("bundled");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(bundled.join("models")).unwrap();
        let spec = serde_json::json!({"name":"model.gguf","bytes":5,"sha256":format!("{:x}",Sha256::digest(b"right"))});
        std::fs::write(cache.join("model.gguf"), b"wrong").unwrap();
        std::fs::write(bundled.join("models/model.gguf"), b"right").unwrap();
        assert_eq!(
            resolve(&cache, &bundled, &spec, &AtomicBool::new(false)).unwrap(),
            bundled.join("models/model.gguf")
        );
        assert!(resolve(&cache, &bundled, &spec, &AtomicBool::new(true)).is_err());
    }
    #[test]
    fn invalid_or_cancelled_files_are_never_qualified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model");
        std::fs::write(&path, b"wrong").unwrap();
        let valid =
            serde_json::json!({"bytes":5,"sha256":format!("{:x}",Sha256::digest(b"right"))});
        assert!(verified(&path, &valid, &AtomicBool::new(false)).is_err());
        std::fs::write(&path, b"right").unwrap();
        assert!(verified(&path, &valid, &AtomicBool::new(true)).is_err());
        assert!(verified(&path, &valid, &AtomicBool::new(false)).is_ok());
        assert!(spec("../../anything").is_err());
    }
    #[test]
    fn interrupted_download_does_not_activate_partial_or_delete_bundled() {
        let dir = tempfile::tempdir().unwrap();
        let bundled = dir.path().join("bundled");
        let cache = dir.path().join("cache");
        std::fs::create_dir_all(bundled.join("models")).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        let s = spec("small").unwrap();
        let name = s["name"].as_str().unwrap();
        std::fs::write(bundled.join("models").join(name), b"bundled").unwrap();
        std::fs::write(cache.join(name).with_extension("part"), b"incomplete").unwrap();
        let manager = ModelManager::new(cache, bundled.clone());
        assert!(
            !manager
                .catalog(&bundled)
                .iter()
                .find(|m| m.id == "small")
                .unwrap()
                .installed
        );
        manager.remove("small").unwrap();
        assert_eq!(
            std::fs::read(bundled.join("models").join(name)).unwrap(),
            b"bundled"
        );
    }
}
