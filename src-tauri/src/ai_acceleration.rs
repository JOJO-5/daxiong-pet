//! Optional LiteRT component: explicit download, fixed sources, isolated install.
use crate::ai_store::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
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

const SOURCES: &str = include_str!("../../native/litert/runtime-sources.json");
const WORKERS: &str = include_str!("../../native/litert/worker-catalog.json");
pub fn target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x64"),
        ("windows", "x86_64") => Some("windows-x64"),
        ("macos", "aarch64") => Some("macos-arm64"),
        ("macos", "x86_64") => Some("macos-x64"),
        _ => None,
    }
}
#[derive(Clone, Serialize)]
pub struct View {
    pub available: bool,
    pub installed: bool,
    pub model_installed: bool,
    pub component_bytes: u64,
    pub model_bytes: u64,
    pub busy: bool,
    pub received: u64,
    pub total: u64,
    pub status: String,
}
#[derive(Clone, Deserialize)]
struct Source {
    url: String,
    bytes: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Installed {
    files: std::collections::BTreeMap<String, SourceHash>,
}
#[derive(Serialize, Deserialize)]
struct SourceHash {
    bytes: u64,
    sha256: String,
}
pub struct Manager {
    pub root: PathBuf,
    state: Mutex<View>,
    token: Mutex<Option<Arc<AtomicBool>>>,
}
pub struct Resources {
    pub worker: PathBuf,
    pub library: PathBuf,
    pub model: PathBuf,
}
fn specs() -> Result<(Value, Value, Value)> {
    let target = target().ok_or("此平台尚不支持可选加速。")?;
    let sources: Value = serde_json::from_str(SOURCES).map_err(|_| "加速清单无效。")?;
    let workers: Value = serde_json::from_str(WORKERS).map_err(|_| "加速清单无效。")?;
    let worker = workers["workers"][target].clone();
    if worker["sha256"].as_str().is_none() {
        return Err("本平台加速组件尚未发布，继续使用CPU。".into());
    }
    Ok((
        sources[target].clone(),
        worker,
        sources["test_model"].clone(),
    ))
}
fn source(value: &Value) -> Result<Source> {
    serde_json::from_value(value.clone()).map_err(|_| "加速来源无效。".into())
}
fn checked(token: &AtomicBool) -> Result<()> {
    if token.load(Ordering::SeqCst) {
        Err("加速下载已停止。".into())
    } else {
        Ok(())
    }
}
fn digest(path: &Path, token: &AtomicBool) -> Result<SourceHash> {
    let mut f = std::fs::File::open(path).map_err(|_| "加速文件不存在。")?;
    let mut sha = Sha256::new();
    let mut bytes = 0;
    let mut buf = [0u8; 256 * 1024];
    loop {
        checked(token)?;
        let n = f.read(&mut buf).map_err(|_| "读取加速文件失败。")?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        sha.update(&buf[..n]);
    }
    Ok(SourceHash {
        bytes,
        sha256: format!("{:x}", sha.finalize()),
    })
}
fn verify(path: &Path, s: &Source, token: &AtomicBool) -> Result<()> {
    let h = digest(path, token)?;
    if h.bytes != s.bytes || h.sha256 != s.sha256 {
        Err("加速文件校验失败，请修复下载。".into())
    } else {
        Ok(())
    }
}
impl Manager {
    pub fn new(root: PathBuf) -> Self {
        // Previous incomplete installs/downloads are never treated as available.
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with(".stage-") {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        for name in ["runtime.part", "worker.part", "Qwen3.5-0.8B_int8.part"] {
            let _ = std::fs::remove_file(root.join(name));
        }
        let manager = Self {
            root,
            state: Mutex::new(View {
                available: specs().is_ok(),
                installed: false,
                model_installed: false,
                component_bytes: 0,
                model_bytes: 0,
                busy: false,
                received: 0,
                total: 0,
                status: "可选组件默认关闭，下载后再启用；GPU模式仍为实验性。".into(),
            }),
            token: Mutex::new(None),
        };
        if let Ok(dest) = manager.component() {
            let backup = manager.root.join(".previous-component");
            if !dest.exists() && backup.is_dir() {
                let _ = std::fs::rename(backup, dest);
            }
        }
        manager
    }
    fn component(&self) -> Result<PathBuf> {
        let (_, w, _) = specs()?;
        let hash = w["sha256"].as_str().ok_or("组件清单无效。")?;
        Ok(self.root.join(format!("component-{}", &hash[..16])))
    }
    pub fn model_path(&self) -> PathBuf {
        self.root.join("Qwen3.5-0.8B_int8.litertlm")
    }
    pub fn view(&self) -> View {
        let mut v = self.state.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Ok((r, w, m)) = specs() {
            v.component_bytes = r["bytes"].as_u64().unwrap_or(0) + w["bytes"].as_u64().unwrap_or(0);
            v.model_bytes = m["bytes"].as_u64().unwrap_or(0);
            v.installed = self
                .component()
                .is_ok_and(|p| p.join("installed.json").is_file());
            v.model_installed = self
                .model_path()
                .metadata()
                .is_ok_and(|x| x.len() == v.model_bytes);
        }
        v
    }
    pub fn cancel(&self) {
        if let Some(t) = self
            .token
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            t.store(true, Ordering::SeqCst)
        }
    }
    pub fn download(self: &Arc<Self>) -> Result<()> {
        let (r, w, m) = specs()?;
        let total = source(&r)?.bytes + source(&w)?.bytes + source(&m)?.bytes;
        let t = Arc::new(AtomicBool::new(false));
        {
            let mut v = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if v.busy {
                return Err("已有加速下载任务。".into());
            }
            v.busy = true;
            v.received = 0;
            v.total = total;
            v.status = "正在校验已有文件或下载加速组件与兼容模型。".into();
            *self.token.lock().unwrap_or_else(|p| p.into_inner()) = Some(t.clone());
        }
        let manager = self.clone();
        if std::thread::Builder::new()
            .name("pet-acceleration-download".into())
            .spawn(move || {
                let result = manager.install(&r, &w, &m, &t);
                let mut v = manager.state.lock().unwrap_or_else(|p| p.into_inner());
                v.busy = false;
                v.status = match result {
                    Ok(()) => "下载校验完成，可启用实验性GPU模式；启用前将试运行。".into(),
                    Err(e) => e,
                };
                *manager.token.lock().unwrap_or_else(|p| p.into_inner()) = None;
            })
            .is_err()
        {
            self.state.lock().unwrap_or_else(|p| p.into_inner()).busy = false;
            *self.token.lock().unwrap_or_else(|p| p.into_inner()) = None;
            return Err("无法启动加速下载任务。".into());
        }
        Ok(())
    }
    fn fetch(&self, s: &Source, path: &Path, token: &AtomicBool, offset: u64) -> Result<()> {
        checked(token)?;
        if verify(path, s, token).is_ok() {
            self.progress(offset + s.bytes);
            return Ok(());
        }
        checked(token)?;
        if !s.url.starts_with("https://") {
            return Err("加速下载来源必须使用HTTPS。".into());
        }
        let part = path.with_extension("part");
        let result = tauri::async_runtime::block_on(async {
            let future = async {
                let client = tokio::task::spawn_blocking(|| {
                    reqwest::Client::builder()
                        .connect_timeout(Duration::from_secs(15))
                        .timeout(Duration::from_secs(3600))
                        .build()
                })
                .await
                .map_err(|_| "下载连接初始化失败。")?
                .map_err(|_| "下载连接初始化失败。")?;
                client
                    .get(&s.url)
                    .send()
                    .await
                    .map_err(|_| "加速下载连接失败，可重试。")
            };
            tokio::pin!(future);
            let response = loop {
                checked(token)?;
                tokio::select! {r=&mut future=>break r?,_=tokio::time::sleep(Duration::from_millis(25))=>{}}
            };
            if !response.status().is_success() {
                return Err("加速来源暂不可用，可重试。".into());
            }
            let mut stream = response.bytes_stream();
            let mut out =
                std::fs::File::create(&part).map_err(|_| "加速目录不可写或磁盘空间不足。")?;
            let mut sha = Sha256::new();
            let mut bytes = 0u64;
            loop {
                checked(token)?;
                let next = tokio::select! {c=stream.next()=>c,_=tokio::time::sleep(Duration::from_millis(25))=>continue};
                let Some(chunk) = next else { break };
                let chunk = chunk.map_err(|_| "加速下载中断，可重试。")?;
                bytes += chunk.len() as u64;
                if bytes > s.bytes {
                    return Err("加速下载超过固定大小，已停止。".into());
                }
                out.write_all(&chunk)
                    .map_err(|_| "磁盘空间不足，下载已停止。")?;
                sha.update(&chunk);
                self.progress(offset + bytes);
            }
            if bytes != s.bytes || format!("{:x}", sha.finalize()) != s.sha256 {
                return Err("加速下载校验失败，未安装。".into());
            }
            out.sync_all().map_err(|_| "保存加速下载失败。")?;
            drop(out);
            checked(token)?;
            if path.exists() {
                std::fs::remove_file(path).map_err(|_| "无法替换损坏的下载。")?;
            }
            std::fs::rename(&part, path).map_err(|_| "无法保存加速下载。".into())
        });
        if result.is_err() {
            let _ = std::fs::remove_file(part);
        }
        result
    }
    fn progress(&self, received: u64) {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .received = received;
    }
    fn install(&self, r: &Value, w: &Value, m: &Value, token: &AtomicBool) -> Result<()> {
        std::fs::create_dir_all(&self.root).map_err(|_| "无法创建加速目录。")?;
        let runtime = source(r)?;
        let worker = source(w)?;
        let model = source(m)?;
        let sdk = self.root.join("runtime.zip");
        let shim = self.root.join("worker.zip");
        self.fetch(&runtime, &sdk, token, 0)?;
        self.fetch(&worker, &shim, token, runtime.bytes)?;
        self.fetch(
            &model,
            &self.model_path(),
            token,
            runtime.bytes + worker.bytes,
        )?;
        checked(token)?;
        let stage = tempfile::Builder::new()
            .prefix(".stage-")
            .tempdir_in(&self.root)
            .map_err(|_| "无法创建加速安装目录。")?;
        let files: Vec<String> =
            serde_json::from_value(r["files"].clone()).map_err(|_| "平台库清单无效。")?;
        extract_sdk(&sdk, stage.path(), &files, token)?;
        extract_worker(
            &shim,
            stage.path(),
            w["name"].as_str().ok_or("运行器清单无效。")?,
            token,
        )?;
        std::fs::create_dir_all(stage.path().join("licenses")).map_err(|_| "无法保存许可证。")?;
        std::fs::write(
            stage.path().join("licenses/LiteRT-LM-LICENSE.txt"),
            include_str!("../../native/litert/LiteRT-LM-LICENSE.txt"),
        )
        .map_err(|_| "无法保存许可证。")?;
        std::fs::write(
            stage.path().join("licenses/QWEN35-LICENSE.txt"),
            include_str!("../../scripts/ai-prototype/QWEN35-LICENSE.txt"),
        )
        .map_err(|_| "无法保存许可证。")?;
        std::fs::write(stage.path().join("licenses/SOURCES.txt"),format!("Runtime: {}\nModel: {}\nWorker: {}\nRuntime downloaded directly from upstream, not redistributed inside the desktop installer.\n",runtime.url,model.url,worker.url)).map_err(|_|"无法保存来源。")?;
        let mut hashes = std::collections::BTreeMap::new();
        for name in files
            .iter()
            .chain(std::iter::once(&w["name"].as_str().unwrap().to_owned()))
        {
            hashes.insert(name.clone(), digest(&stage.path().join(name), token)?);
        }
        std::fs::write(
            stage.path().join("installed.json"),
            serde_json::to_vec(&Installed { files: hashes }).map_err(|_| "组件清单写入失败。")?,
        )
        .map_err(|_| "组件清单写入失败。")?;
        checked(token)?;
        let dest = self.component()?;
        let backup = self.root.join(".previous-component");
        if backup.exists() {
            std::fs::remove_dir_all(&backup).map_err(|_| "无法清理旧组件。")?;
        }
        if dest.exists() {
            std::fs::rename(&dest, &backup).map_err(|_| "无法修复已有组件，请先关闭加速。")?;
        }
        if std::fs::rename(stage.path(), &dest).is_err() {
            if backup.exists() {
                let _ = std::fs::rename(&backup, &dest);
            }
            return Err("组件安装失败，原组件已保留。".into());
        }
        let _ = std::fs::remove_dir_all(backup);
        let _ = std::fs::remove_file(sdk);
        let _ = std::fs::remove_file(shim);
        Ok(())
    }
    pub fn resources(&self, token: &AtomicBool) -> Result<Resources> {
        if self.view().busy {
            return Err("加速组件正在下载，暂用CPU。".into());
        }
        let (r, w, m) = specs()?;
        let p = self.component()?;
        let manifest: Installed = serde_json::from_slice(
            &std::fs::read(p.join("installed.json")).map_err(|_| "加速组件未安装。")?,
        )
        .map_err(|_| "加速组件清单损坏。")?;
        let names: Vec<String> =
            serde_json::from_value(r["files"].clone()).map_err(|_| "平台库清单无效。")?;
        let worker = w["name"].as_str().ok_or("运行器清单无效。")?;
        for name in names
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(worker))
        {
            let expected = manifest.files.get(name).ok_or("加速组件不完整。")?;
            let actual = digest(&p.join(name), token)?;
            if actual.bytes != expected.bytes || actual.sha256 != expected.sha256 {
                return Err("加速组件损坏，请修复下载。".into());
            }
        }
        verify(&self.model_path(), &source(&m)?, token)?;
        Ok(Resources {
            worker: p.join(worker),
            library: p.join(&names[0]),
            model: self.model_path(),
        })
    }
    pub fn remove(&self, model: bool) -> Result<()> {
        if self.view().busy {
            return Err("请先取消加速下载。".into());
        }
        if model {
            let p = self.model_path();
            if p.exists() {
                std::fs::remove_file(p).map_err(|_| "删除加速模型失败。")?;
            }
        } else {
            let p = self.component()?;
            if p.exists() {
                std::fs::remove_dir_all(p).map_err(|_| "删除加速组件失败。")?;
            }
        }
        Ok(())
    }
}
fn extract_sdk(archive: &Path, out: &Path, names: &[String], token: &AtomicBool) -> Result<()> {
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(archive).map_err(|_| "平台库下载不存在。")?)
            .map_err(|_| "平台库压缩包损坏。")?;
    for name in names {
        let mut matches = Vec::new();
        for i in 0..zip.len() {
            let f = zip.by_index(i).map_err(|_| "平台库压缩包损坏。")?;
            if Path::new(f.name()).file_name().and_then(|p| p.to_str()) == Some(name.as_str()) {
                matches.push(i)
            }
        }
        if matches.len() != 1 {
            return Err("平台库文件不唯一或缺失。".into());
        }
        let mut file = zip.by_index(matches[0]).map_err(|_| "平台库压缩包损坏。")?;
        copy_entry(&mut file, &out.join(name), token)?;
    }
    Ok(())
}
fn extract_worker(archive: &Path, out: &Path, name: &str, token: &AtomicBool) -> Result<()> {
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(archive).map_err(|_| "运行器下载不存在。")?)
            .map_err(|_| "运行器压缩包损坏。")?;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|_| "运行器压缩包损坏。")?;
        let path = f
            .enclosed_name()
            .ok_or("运行器压缩包路径无效。")?
            .to_owned();
        if f.is_dir() {
            continue;
        }
        if path == Path::new(name) {
            count += 1;
            copy_entry(&mut f, &out.join(name), token)?;
        } else if path.starts_with("licenses") {
            copy_entry(&mut f, &out.join(path), token)?;
        } else {
            return Err("运行器包含非预期文件。".into());
        }
    }
    if count != 1 {
        return Err("运行器文件不唯一或缺失。".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out.join(name), std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "设置运行器权限失败。")?;
    }
    Ok(())
}
fn copy_entry(file: &mut zip::read::ZipFile<'_>, path: &Path, token: &AtomicBool) -> Result<()> {
    if file.size() > 300 * 1024 * 1024 || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
    {
        return Err("组件文件大小或类型无效。".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "组件目录不可写。")?;
    }
    let mut out = std::fs::File::create(path).map_err(|_| "组件目录不可写。")?;
    let mut buf = [0u8; 256 * 1024];
    loop {
        checked(token)?;
        let n = file.read(&mut buf).map_err(|_| "解压组件失败。")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|_| "磁盘空间不足。")?;
    }
    out.sync_all().map_err(|_| "保存组件失败。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(path: &Path, files: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        for (name, data) in files {
            zip.start_file(*name, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    #[test]
    fn corruption_and_cancel_never_verify() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("model");
        std::fs::write(&p, b"correct").unwrap();
        let token = AtomicBool::new(false);
        let h = digest(&p, &token).unwrap();
        let source = Source {
            url: "https://example.invalid/model".into(),
            bytes: h.bytes,
            sha256: h.sha256,
        };
        assert!(verify(&p, &source, &token).is_ok());
        std::fs::write(&p, b"corrupt").unwrap();
        assert!(verify(&p, &source, &token).is_err());
        token.store(true, Ordering::SeqCst);
        assert!(digest(&p, &token).is_err());
    }
    #[test]
    fn worker_archive_rejects_traversal_and_missing_binary() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("worker.zip");
        let out = d.path().join("out");
        std::fs::create_dir(&out).unwrap();
        archive(&p, &[("../escape", b"bad")]);
        assert!(extract_worker(&p, &out, "worker", &AtomicBool::new(false)).is_err());
        assert!(!d.path().join("escape").exists());
        archive(&p, &[("licenses/license", b"MIT")]);
        assert!(extract_worker(&p, &out, "worker", &AtomicBool::new(false)).is_err());
    }
    #[test]
    fn sdk_only_extracts_unique_named_libraries() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sdk.zip");
        let names = vec!["runtime.so".into()];
        archive(&p, &[("a/runtime.so", b"one"), ("b/runtime.so", b"two")]);
        assert!(extract_sdk(&p, d.path(), &names, &AtomicBool::new(false)).is_err());
        archive(&p, &[("a/runtime.so", b"one"), ("../../escape", b"evil")]);
        extract_sdk(&p, d.path(), &names, &AtomicBool::new(false)).unwrap();
        assert_eq!(std::fs::read(d.path().join("runtime.so")).unwrap(), b"one");
    }
}
