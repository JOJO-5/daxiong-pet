//! CPU model lives in a child process; no inference or HTTP work on the pet engine.
use crate::ai_store::{Result, Settings};
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

const MODEL_MANIFEST: &str = include_str!("../../scripts/ai-prototype/default-model.json");
#[cfg(test)]
pub const CONTEXT: usize = 4096;
#[derive(Clone)]
pub struct Assets {
    pub root: PathBuf,
}
#[derive(Clone, Serialize)]
pub struct AssetStatus {
    pub available: bool,
    pub model: String,
    pub note: String,
}
impl Assets {
    fn spec(&self) -> Value {
        serde_json::from_str::<Value>(MODEL_MANIFEST).expect("compiled model manifest")
            ["default_text_model"]
            .clone()
    }
    fn model_path(&self) -> PathBuf {
        self.root
            .join("models")
            .join(self.spec()["name"].as_str().expect("model name"))
    }
    fn binary_path(&self) -> PathBuf {
        self.root.join("runtime").join(if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        })
    }
    pub fn status(&self) -> AssetStatus {
        let available = self.model_path().is_file() && self.binary_path().is_file();
        AssetStatus {
            available,
            model: "Qwen3.5 2B · CPU".into(),
            note: if available {
                "本地资源已找到，首次聊天将校验并加载模型。"
            } else {
                "本地AI资源尚未安装。这一开发预览需先准备模型与运行器。"
            }
            .into(),
        }
    }
}
#[derive(Clone)]
pub struct Endpoint {
    base: String,
    key: String,
    context: usize,
    timeout: u64,
}
struct Running {
    child: Child,
    endpoint: Endpoint,
    threads: u8,
    vision: bool,
    model_id: String,
    context: usize,
}
pub struct Runtime {
    assets: Assets,
    cache_root: PathBuf,
    verified_model: Mutex<Option<(String, PathBuf)>>,
    running: Mutex<Option<Running>>,
    idle_seconds: std::sync::atomic::AtomicU64,
    vision_verified: Mutex<Option<PathBuf>>,
    last_used: Mutex<Instant>,
    #[cfg(target_os = "linux")]
    launcher: Mutex<Option<std::sync::mpsc::Sender<Launch>>>,
}
#[cfg(target_os = "linux")]
type Launch = (Command, std::sync::mpsc::SyncSender<std::io::Result<Child>>);
fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::SeqCst) {
        Err("生成已停止。".into())
    } else {
        Ok(())
    }
}

impl Runtime {
    #[cfg(test)]
    pub fn new(assets: Assets) -> Self {
        let cache = assets.root.join("cache");
        Self::new_with_cache(assets, cache)
    }
    pub fn new_with_cache(assets: Assets, cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            verified_model: Mutex::new(None),
            assets,
            running: Mutex::new(None),
            idle_seconds: std::sync::atomic::AtomicU64::new(300),
            vision_verified: Mutex::new(None),
            last_used: Mutex::new(Instant::now()),
            #[cfg(target_os = "linux")]
            launcher: Mutex::new(None),
        }
    }
    fn spawn_child(&self, command: Command) -> Result<Child> {
        #[cfg(target_os = "linux")]
        {
            // PDEATHSIG follows the spawning *thread*. Keep that thread alive across replies.
            let mut launcher = self.launcher.lock().unwrap_or_else(|p| p.into_inner());
            if launcher.is_none() {
                let (tx, rx) = std::sync::mpsc::channel::<Launch>();
                std::thread::Builder::new()
                    .name("pet-ai-launcher".into())
                    .spawn(move || {
                        while let Ok((mut command, reply)) = rx.recv() {
                            if let Err(std::sync::mpsc::SendError(Ok(mut child))) =
                                reply.send(command.spawn())
                            {
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                        }
                    })
                    .map_err(|_| "无法启动本地模型管理线程。")?;
                *launcher = Some(tx);
            }
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            launcher
                .as_ref()
                .expect("initialized launcher")
                .send((command, tx))
                .map_err(|_| "本地模型管理线程已退出。")?;
            return rx
                .recv()
                .map_err(|_| "本地模型管理线程已退出。")?
                .map_err(|_| "启动本地模型失败，请检查运行器及其依赖。".into());
        }
        #[cfg(not(target_os = "linux"))]
        {
            let mut command = command;
            command
                .spawn()
                .and_then(crate::ai_process::own)
                .map_err(|_| "启动本地模型失败，请检查运行器及其依赖。".into())
        }
    }
    pub fn assets(&self, settings: &Settings) -> AssetStatus {
        let mut status = self.assets.status();
        if let Ok(spec) = crate::ai_models::spec(&settings.local_model) {
            if let Some(name) = spec["name"].as_str() {
                status.available = self.assets.binary_path().is_file()
                    && (self.cache_root.join(name).is_file()
                        || self.assets.root.join("models").join(name).is_file())
                    && (!cfg!(target_os = "macos")
                        || self.assets.root.join("runtime/daxiong-ai-guard").is_file());
                status.note = if status.available {
                    "本地资源已找到，首次聊天将校验并加载模型。"
                } else {
                    "本地模型尚未准备好，请在性格与设置中下载模型；运行器缺失时请修复安装。"
                }
                .into();
            }
        }
        status
    }
    pub fn loaded(&self) -> bool {
        let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if guard
            .as_mut()
            .is_some_and(|r| r.child.try_wait().ok().flatten().is_some())
        {
            *guard = None;
        }
        guard.is_some()
    }
    pub fn stop(&self) {
        let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut running) = guard.take() {
            let _ = crate::ai_process::kill(&mut running.child);
            let _ = running.child.wait();
        }
    }
    pub fn unload_if_idle(&self) {
        if self
            .last_used
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .elapsed()
            >= Duration::from_secs(self.idle_seconds.load(Ordering::Relaxed))
        {
            self.stop();
        }
    }
    pub fn touch(&self) {
        *self.last_used.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
    }
    pub fn ensure(&self, settings: &Settings, cancelled: &AtomicBool) -> Result<Endpoint> {
        self.ensure_mode(settings, cancelled, false)
    }
    pub fn ensure_mode(
        &self,
        settings: &Settings,
        cancelled: &AtomicBool,
        vision: bool,
    ) -> Result<Endpoint> {
        check_cancel(cancelled)?;
        let model_id = if vision { "2b" } else { &settings.local_model };
        self.touch();
        self.idle_seconds
            .store(settings.idle_seconds, Ordering::Relaxed);
        {
            let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(r) = guard.as_mut() {
                if r.threads == settings.threads
                    && r.vision == vision
                    && r.model_id == model_id
                    && r.context == settings.local_context
                    && r.child
                        .try_wait()
                        .map_err(|_| "无法读取模型进程状态。")?
                        .is_none()
                {
                    return Ok(r.endpoint.clone());
                }
            }
        }
        self.stop();
        let model_spec = crate::ai_models::spec(model_id)?;
        let model = {
            let mut verified = self
                .verified_model
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if let Some((id, path)) = verified
                .as_ref()
                .filter(|(id, path)| id == model_id && path.is_file())
            {
                let _ = id;
                path.clone()
            } else {
                let path = crate::ai_models::resolve(
                    &self.cache_root,
                    &self.assets.root,
                    &model_spec,
                    cancelled,
                )?;
                *verified = Some((model_id.into(), path.clone()));
                path
            }
        };
        let projector = if vision {
            let mut verified = self
                .vision_verified
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if let Some(path) = verified.as_ref().filter(|p| p.is_file()) {
                Some(path.clone())
            } else {
                let path = crate::ai_models::resolve(
                    &self.cache_root,
                    &self.assets.root,
                    &crate::ai_models::spec("vision")?,
                    cancelled,
                )?;
                *verified = Some(path.clone());
                Some(path)
            }
        } else {
            None
        };
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "无法为本地模型分配端口。")?;
        let port = listener
            .local_addr()
            .map_err(|_| "无法为本地模型分配端口。")?
            .port();
        let endpoint = Endpoint {
            base: format!("http://127.0.0.1:{port}"),
            key: uuid::Uuid::new_v4().to_string(),
            context: settings.local_context,
            timeout: settings.timeout_seconds,
        };
        let mut command = crate::ai_process::command(
            &self.assets.binary_path(),
            &self.assets.root.join("runtime"),
        )?;
        command
            .args(["-m"])
            .arg(model)
            .args(["--host", "127.0.0.1", "--port"])
            .arg(port.to_string())
            .args(["--api-key"])
            .arg(&endpoint.key)
            .args(["-t"])
            .arg(settings.threads.to_string())
            .args(["-tb"])
            .arg(settings.threads.to_string())
            .arg("-c")
            .arg(settings.local_context.to_string())
            .args(["-np", "1", "-ngl", "0", "--jinja", "--no-warmup"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if vision {
            command
                .arg("--mmproj")
                .arg(projector.as_ref().ok_or("视觉组件缺失。")?)
                .args(["--image-min-tokens", "256", "--image-max-tokens", "256"]);
            #[cfg(target_os = "macos")]
            command.args(["--mmproj-device", "BLAS"]); // System CPU Accelerate, not Metal.
            #[cfg(not(target_os = "macos"))]
            command.arg("--no-mmproj-offload");
        } else {
            command.arg("--no-mmproj");
        }
        #[cfg(target_os = "macos")]
        command.env("VECLIB_MAXIMUM_THREADS", settings.threads.to_string());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            let parent = std::process::id() as libc::pid_t;
            // No allocation/locks after fork; kill the child if the desktop app crashes.
            unsafe {
                command.pre_exec(move || {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                    }
                    Ok(())
                });
            }
        }
        {
            let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
            check_cancel(cancelled)?;
            drop(listener);
            let child = self.spawn_child(command)?;
            *guard = Some(Running {
                child,
                endpoint: endpoint.clone(),
                threads: settings.threads,
                vision,
                model_id: model_id.into(),
                context: settings.local_context,
            });
        }
        let health = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|_| "无法连接本地模型。")?;
        let start = Instant::now();
        loop {
            check_cancel(cancelled)?;
            if !self.loaded() {
                return Err("本地模型进程已退出，请检查资源或内存后重试。".into());
            }
            if health
                .get(format!("{}/health", endpoint.base))
                .bearer_auth(&endpoint.key)
                .send()
                .is_ok_and(|r| r.status().is_success())
            {
                return Ok(endpoint);
            }
            if start.elapsed() > Duration::from_secs(120) {
                self.stop();
                return Err("模型加载超时，请稍后重试。".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
pub fn fit_context(
    messages: Vec<Value>,
    output_tokens: usize,
    count: impl FnMut(&[Value]) -> Result<usize>,
) -> Result<Vec<Value>> {
    fit_context_limit(messages, output_tokens, CONTEXT, count)
}
fn fit_context_limit(
    mut messages: Vec<Value>,
    output_tokens: usize,
    context: usize,
    mut count: impl FnMut(&[Value]) -> Result<usize>,
) -> Result<Vec<Value>> {
    loop {
        if count(&messages)? + output_tokens + 128 <= context {
            return Ok(messages);
        }
        if messages.len() <= 2 {
            return Err("这条消息超过本地模型的上下文容量，请缩短后重试。".into());
        }
        // Always remove a complete oldest user/assistant pair, never the current message.
        messages.drain(1..3);
    }
}
fn post_json(client: &Client, endpoint: &Endpoint, path: &str, body: &Value) -> Result<Value> {
    client
        .post(format!("{}{path}", endpoint.base))
        .bearer_auth(&endpoint.key)
        .json(body)
        .send()
        .map_err(|_| "本地模型连接中断，请重试。")?
        .error_for_status()
        .map_err(|_| "本地模型拒绝了请求，请重试。")?
        .json()
        .map_err(|_| "本地模型返回了无效数据。".into())
}
pub fn generate(
    endpoint: &Endpoint,
    messages: Vec<Value>,
    output_tokens: usize,
    cancelled: &AtomicBool,
    mut on_delta: impl FnMut(&str) -> Result<()>,
) -> Result<String> {
    let client = Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(endpoint.timeout))
        .build()
        .map_err(|_| "本地模型连接失败。")?;
    let messages = fit_context_limit(messages, output_tokens, endpoint.context, |m| {
        check_cancel(cancelled)?;
        let rendered = post_json(
            &client,
            endpoint,
            "/apply-template",
            &json!({"messages":m,"add_generation_prompt":true,"chat_template_kwargs":{"enable_thinking":false}}),
        )?;
        let prompt = rendered["prompt"]
            .as_str()
            .ok_or("模型没有返回可计算的上下文。")?;
        let tokens = post_json(
            &client,
            endpoint,
            "/tokenize",
            &json!({"content":prompt,"add_special":false}),
        )?;
        let images = m
            .iter()
            .filter_map(|x| x["content"].as_array())
            .flatten()
            .filter(|x| x["type"] == "image_url")
            .count();
        tokens["tokens"]
            .as_array()
            .map(|t| t.len() + images * 1024)
            .ok_or_else(|| "模型没有返回有效token数。".into())
    })?;
    check_cancel(cancelled)?;
    let response=client.post(format!("{}/v1/chat/completions",endpoint.base)).bearer_auth(&endpoint.key)
        .json(&json!({"messages":messages,"stream":true,"max_tokens":output_tokens,"temperature":0.6,"top_p":0.9,"top_k":40,"min_p":0.0,"repeat_penalty":1.05,"cache_prompt":true,"chat_template_kwargs":{"enable_thinking":false}}))
        .send().map_err(|_|"本地模型连接中断，请重试。")?.error_for_status().map_err(|_|"本地模型无法生成这次回复。")?;
    read_stream(BufReader::new(response), cancelled, &mut on_delta)
}
fn read_stream(
    mut reader: impl BufRead,
    cancelled: &AtomicBool,
    on_delta: &mut impl FnMut(&str) -> Result<()>,
) -> Result<String> {
    let mut reply = String::new();
    let mut line = Vec::new();
    let mut done = false;
    let mut finished = false;
    loop {
        check_cancel(cancelled)?;
        line.clear();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|_| "回复连接中断，已保留收到的文字。")?;
        if n == 0 {
            break;
        }
        if line.len() > 65536 {
            return Err("本地模型返回数据过长。".into());
        }
        let text = std::str::from_utf8(&line)
            .map_err(|_| "本地模型返回了无效文字。")?
            .trim();
        let Some(data) = text.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            done = true;
            break;
        }
        let event: Value = serde_json::from_str(data).map_err(|_| "本地模型回复格式错误。")?;
        if event.get("error").is_some() {
            return Err("本地模型生成失败，已保留收到的文字。".into());
        }
        if let Some(choices) = event["choices"].as_array() {
            for c in choices {
                if let Some(part) = c["delta"]["content"].as_str() {
                    if !part.is_empty() {
                        if reply.len() + part.len() > 16384 {
                            return Err("回复过长，已停止生成。".into());
                        }
                        reply.push_str(part);
                        on_delta(part)?;
                    }
                }
                if let Some(reason) = c["finish_reason"].as_str() {
                    if !["stop", "length"].contains(&reason) {
                        return Err("本地模型未完成文字回复。".into());
                    }
                    finished = true;
                }
            }
        }
    }
    check_cancel(cancelled)?;
    if !done || !finished {
        return Err("回复连接中断，已保留收到的文字。".into());
    }
    if reply.trim().is_empty() {
        return Err("这次没有生成文字，请重试。".into());
    }
    Ok(reply)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn model_owner_thread_survives_the_reply_worker() {
        use std::os::unix::process::CommandExt;
        let runtime = std::sync::Arc::new(Runtime::new(Assets {
            root: PathBuf::new(),
        }));
        let worker_runtime = runtime.clone();
        let mut child = std::thread::spawn(move || {
            let mut command = Command::new("/bin/sleep");
            command.arg("10");
            unsafe {
                command.pre_exec(|| {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            worker_runtime.spawn_child(command).unwrap()
        })
        .join()
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let alive = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            alive,
            "reply thread exit must not kill the reusable model process"
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn idle_unload_reaps_a_real_child_but_keeps_a_recent_one() {
        let runtime = Runtime::new(Assets {
            root: PathBuf::new(),
        });
        let mut command = Command::new("/bin/sleep");
        command.arg("10");
        let child = runtime.spawn_child(command).unwrap();
        let pid = child.id();
        *runtime.running.lock().unwrap() = Some(Running {
            child,
            endpoint: Endpoint {
                base: String::new(),
                key: String::new(),
                context: 4096,
                timeout: 180,
            },
            threads: 2,
            vision: false,
            model_id: "2b".into(),
            context: 4096,
        });
        runtime.unload_if_idle();
        assert!(runtime.loaded());
        *runtime.last_used.lock().unwrap() = Instant::now() - Duration::from_secs(301);
        runtime.unload_if_idle();
        assert!(!runtime.loaded());
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "child must be reaped, not left as a zombie"
        );
    }
    #[test]
    fn streams_unicode_and_distinguishes_truncation_from_broken_transport() {
        let flag = AtomicBool::new(false);
        let mut received = String::new();
        let source="data: {\"choices\":[{\"delta\":{\"content\":\"陪着你🐶\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n";
        let value = read_stream(std::io::Cursor::new(source), &flag, &mut |d| {
            received.push_str(d);
            Ok(())
        })
        .unwrap();
        assert_eq!(value, received);
        assert_eq!(value, "陪着你🐶");
        assert!(read_stream(
            std::io::Cursor::new(source.split("data: [DONE]").next().unwrap()),
            &flag,
            &mut |_| Ok(())
        )
        .is_err());
        flag.store(true, Ordering::SeqCst);
        assert!(
            read_stream(std::io::Cursor::new(source), &flag, &mut |_| panic!(
                "late delta"
            ))
            .is_err()
        );
    }
    #[test]
    fn trims_complete_pairs_and_never_silently_truncates_current_input() {
        let messages = vec![
            json!({"role":"system","content":"rules"}),
            json!({"role":"user","content":"old"}),
            json!({"role":"assistant","content":"old reply"}),
            json!({"role":"user","content":"current"}),
        ];
        let result =
            fit_context(messages, 96, |m| Ok(if m.len() > 2 { 4000 } else { 100 })).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1]["content"], "current");
        assert!(fit_context(result, 96, |_| Ok(4000)).is_err());
    }
}
