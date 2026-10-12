//! Parent-owned optional native process. Startup/Stop never run on the pet engine.
use crate::{
    ai_acceleration::Manager,
    ai_store::{Result, Settings},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Serialize)]
pub struct View {
    pub backend: String,
    pub note: String,
}
struct Running {
    child: Child,
    stdin: ChildStdin,
    port: u16,
    threads: u8,
    context: usize,
}
pub struct Runtime {
    running: Mutex<Option<Running>>,
    view: Mutex<View>,
    failed: AtomicBool,
    last_used: Mutex<Instant>,
}
impl Runtime {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(None),
            view: Mutex::new(View {
                backend: "cpu".into(),
                note: "默认使用CPU。".into(),
            }),
            failed: AtomicBool::new(false),
            last_used: Mutex::new(Instant::now()),
        }
    }
    pub fn view(&self) -> View {
        let mut view = self.view.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if running
            .as_mut()
            .is_none_or(|r| r.child.try_wait().ok().flatten().is_some())
            && view.backend == "ml-drift-gpu"
        {
            view.backend = "cpu".into();
            view.note = "加速模型已释放，下次本地聊天会尝试重新加载。".into();
        }
        view
    }
    pub fn reset(&self) {
        self.stop();
        self.failed.store(false, Ordering::SeqCst);
        *self.view.lock().unwrap_or_else(|p| p.into_inner()) = View {
            backend: "cpu".into(),
            note: "默认使用CPU；启用加速后将先试运行。".into(),
        };
    }
    pub fn fallback(&self, reason: &str) {
        self.failed.store(true, Ordering::SeqCst);
        self.stop();
        *self.view.lock().unwrap_or_else(|p| p.into_inner()) = View {
            backend: "cpu".into(),
            note: format!("加速未能运行，已回到CPU：{reason}"),
        };
    }
    pub fn eligible(&self) -> bool {
        !self.failed.load(Ordering::SeqCst)
    }
    pub fn stop(&self) {
        let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut r) = guard.take() {
            let _ = writeln!(r.stdin, "{{\"op\":\"shutdown\"}}");
            drop(r.stdin);
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(2) {
                if r.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let _ = crate::ai_process::kill(&mut r.child);
            let _ = r.child.wait();
        }
    }
    pub fn unload_if_idle(&self, seconds: u64) {
        if self
            .last_used
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .elapsed()
            >= Duration::from_secs(seconds)
        {
            self.stop()
        }
    }
    fn ensure(
        &self,
        manager: &Manager,
        guard_dir: &Path,
        s: &Settings,
        cancel: &AtomicBool,
    ) -> Result<u16> {
        if cancel.load(Ordering::SeqCst) {
            return Err("生成已停止。".into());
        }
        *self.last_used.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
        {
            let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(r) = running.as_mut() {
                if r.threads == s.threads
                    && r.context == s.local_context
                    && r.child
                        .try_wait()
                        .map_err(|_| "无法检查加速进程。")?
                        .is_none()
                {
                    return Ok(r.port);
                }
            }
        }
        self.stop();
        let resources = manager.resources(cancel)?;
        let mut command = crate::ai_process::command(&resources.worker, guard_dir)?;
        command
            .arg("--library")
            .arg(&resources.library)
            .arg("--model")
            .arg(&resources.model)
            .args(["--backend", "gpu", "--threads"])
            .arg(s.threads.to_string())
            .arg("--context")
            .arg(s.local_context.to_string())
            .current_dir(resources.worker.parent().ok_or("加速运行器目录无效。")?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        // stdin EOF also triggers the worker watchdog if the parent crashes on Unix.
        let mut child =
            crate::ai_process::own(command.spawn().map_err(|_| "加速运行器无法启动。")?)
                .map_err(|_| "加速进程保护失败。")?;
        let stdin = child.stdin.take().ok_or("加速控制通道缺失。")?;
        let stdout = child.stdout.take().ok_or("加速启动通道缺失。")?;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let result = Read::by_ref(&mut reader)
                .take(8192)
                .read_line(&mut line)
                .ok()
                .and_then(|_| serde_json::from_str::<Value>(&line).ok());
            let _ = tx.send(result); // Drain controls so stdout cannot block the native watchdog.
            for line in reader.lines() {
                if line.is_err() {
                    break;
                }
            }
        });
        let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
        *guard = Some(Running {
            child,
            stdin,
            port: 0,
            threads: s.threads,
            context: s.local_context,
        });
        drop(guard);
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if cancel.load(Ordering::SeqCst) {
                self.stop();
                return Err("生成已停止。".into());
            }
            if Instant::now() > deadline {
                self.stop();
                return Err("GPU初始化超时。".into());
            }
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(Some(v))
                    if v["event"] == "ready"
                        && v["requested_backend"] == "gpu"
                        && v["native"] == true =>
                {
                    let port = v["port"]
                        .as_u64()
                        .filter(|p| *p > 0 && *p <= 65535)
                        .ok_or("加速端口无效。")? as u16;
                    let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
                    let r = running.as_mut().ok_or("生成已停止。")?;
                    if v["pid"].as_u64() != Some(r.child.id() as u64) && !cfg!(target_os = "macos")
                    {
                        return Err("加速进程身份无效。".into());
                    }
                    r.port = port;
                    return Ok(port);
                }
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.stop();
                    return Err("此设备无法初始化ML Drift GPU模式。".into());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
    fn local(s: &Settings, port: u16) -> Settings {
        let mut local = s.clone();
        local.service_url = format!("http://127.0.0.1:{port}/v1");
        local.service_model = "daxiong-litert".into();
        local.has_credential = false;
        local.external_context = s.local_context;
        local.remote_screen = false;
        local
    }
    pub fn probe(
        &self,
        manager: &Manager,
        guard_dir: &Path,
        s: &Settings,
        cancel: &AtomicBool,
    ) -> Result<()> {
        self.reset();
        let result = self
            .ensure(manager, guard_dir, s, cancel)
            .and_then(|port| {
                crate::ai_external::generate(
                    &Self::local(s, port),
                    vec![json!({"role":"user","content":"你好，请简短回应。"})],
                    16,
                    cancel,
                    |_| Ok(()),
                )
            })
            .map(|_| ());
        match result {
            Ok(()) => {
                *self.view.lock().unwrap_or_else(|p|p.into_inner())=View{backend:"ml-drift-gpu".into(),note:"ML Drift GPU模式试运行通过（实验性），未承诺性能；仅文字聊天，图片理解和OCR继续CPU。".into()};
                Ok(())
            }
            Err(e) => {
                self.fallback(&e);
                Err(e)
            }
        }
    }
    pub fn generate(
        &self,
        manager: &Manager,
        guard_dir: &Path,
        s: &Settings,
        messages: Vec<Value>,
        output: usize,
        cancel: &AtomicBool,
        delta: impl FnMut(&str) -> Result<()>,
    ) -> Result<String> {
        let port = self.ensure(manager, guard_dir, s, cancel)?;
        let text = crate::ai_external::generate(
            &Self::local(s, port),
            messages,
            output.min(256),
            cancel,
            delta,
        )?;
        *self.view.lock().unwrap_or_else(|p| p.into_inner()) = View {
            backend: "ml-drift-gpu".into(),
            note: "ML Drift GPU模式（实验性）；文字模型0.8B，回答可能较弱。".into(),
        };
        Ok(text)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop()
    }
}
