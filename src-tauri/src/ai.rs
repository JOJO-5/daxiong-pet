use crate::{
    ai_runtime::{self, AssetStatus, Assets, Runtime},
    ai_store::{Message, Personality, Result, Settings, Store},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

pub struct AiState(pub Arc<Service>);
struct Active {
    id: String,
    cancelled: Arc<AtomicBool>,
    message_id: Option<i64>,
    preview: bool,
    partial: String,
}
#[derive(Clone, Serialize)]
pub struct Event {
    request_id: String,
    content: String,
    status: String,
    preview: bool,
    error: Option<String>,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub messages: Vec<Message>,
    pub personality: Personality,
    pub defaults: Personality,
    pub settings: Settings,
    pub assets: AssetStatus,
    pub loaded: bool,
    pub active_request_id: Option<String>,
    pub active_preview: bool,
}
pub struct Service {
    db_path: PathBuf,
    store: Mutex<Option<Store>>,
    runtime: Runtime,
    active: Mutex<Option<Active>>,
    shutdown: AtomicBool,
    generation: AtomicU64,
    chat_focused: AtomicBool,
    chat_hitbox: Mutex<Option<(i32, i32, u32, u32)>>,
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
impl Service {
    pub fn new(db_path: PathBuf, assets: Assets) -> Arc<Self> {
        let service = Arc::new(Self {
            db_path,
            store: Mutex::new(None),
            runtime: Runtime::new(assets),
            active: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            chat_focused: AtomicBool::new(false),
            chat_hitbox: Mutex::new(None),
        });
        let weak = Arc::downgrade(&service);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(5));
            let Some(s) = weak.upgrade() else { break };
            if s.shutdown.load(Ordering::SeqCst) {
                break;
            }
            let gate = s.active.lock().unwrap_or_else(|p| p.into_inner());
            if gate.is_none() {
                s.runtime.unload_if_idle();
            }
            drop(gate);
            // Do not create a database for users who never open AI. Existing databases are maintained.
            if let Some(store) = s.store.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
                let _ = store.cleanup(now());
            };
        });
        service
    }
    fn db<T>(&self, f: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        let mut guard = self.store.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(Store::open(&self.db_path, now())?);
        }
        f(guard.as_mut().expect("opened database"))
    }
    fn snapshot(&self) -> Result<Snapshot> {
        let (id, active_preview) = {
            let guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
            (
                guard.as_ref().map(|a| a.id.clone()),
                guard.as_ref().is_some_and(|a| a.preview),
            )
        };
        let (messages, personality, settings) =
            self.db(|s| Ok((s.messages(now())?, s.profile()?, s.settings()?)))?;
        Ok(Snapshot {
            messages,
            personality,
            defaults: Personality::default(),
            settings,
            assets: self.runtime.assets(),
            loaded: self.runtime.loaded(),
            active_request_id: id,
            active_preview,
        })
    }
    fn save_profile(&self, profile: Personality) -> Result<Personality> {
        let profile = profile.validated()?;
        self.db(|s| s.save_profile(&profile))?;
        Ok(profile)
    }
    fn configure(&self, settings: Settings) -> Result<()> {
        settings.validate()?;
        let mut gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if settings.enabled && gate.is_some() {
            return Err("请先停止回复，再调整CPU占用。".into());
        }
        self.db(|s| s.save_settings(&settings))?;
        if !settings.enabled {
            if let Some(a) = gate.as_mut() {
                a.cancelled.store(true, Ordering::SeqCst);
            }
        }
        self.runtime.stop();
        drop(gate);
        Ok(())
    }
    pub fn cancel(&self) {
        let gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(a) = gate.as_ref() {
            a.cancelled.store(true, Ordering::SeqCst);
        }
        // Hold the gate until the child is killed: another request cannot reuse this process.
        if gate.is_some() {
            self.runtime.stop();
        }
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    pub fn covers_pointer(&self, cursor: (i32, i32)) -> bool {
        self.chat_hitbox
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some_and(|(x, y, w, h)| {
                let (cx, cy) = (i64::from(cursor.0), i64::from(cursor.1));
                cx >= i64::from(x)
                    && cy >= i64::from(y)
                    && cx < i64::from(x) + i64::from(w)
                    && cy < i64::from(y) + i64::from(h)
            })
    }
    pub fn cancel_generation(&self, generation: u64) {
        let gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if self.generation.load(Ordering::SeqCst) == generation {
            if let Some(a) = gate.as_ref() {
                a.cancelled.store(true, Ordering::SeqCst);
                self.runtime.stop();
            }
        }
    }
    fn cancel_request(&self, id: &str) {
        let gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(a) = gate.as_ref().filter(|a| a.id == id) {
            a.cancelled.store(true, Ordering::SeqCst);
            self.runtime.stop();
        }
    }
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.cancel();
        self.runtime.stop();
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(a) = guard.take() {
            if let Some(id) = a.message_id {
                let _ = self.db(|s| s.finish(id, &a.partial, "cancelled"));
            }
        }
    }
    fn start(
        self: &Arc<Self>,
        app: AppHandle,
        id: String,
        text: String,
        preview: Option<Personality>,
    ) -> Result<()> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err("应用正在退出。".into());
        }
        if id.is_empty()
            || id.len() > 64
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("请求编号无效。".into());
        }
        let text = text.trim().to_string();
        if text.is_empty() || text.chars().count() > 2000 {
            return Err("请输入1到2000字的消息。".into());
        }
        let mut gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if gate.is_some() {
            return Err("正在回复，请先等待或停止。".into());
        }
        let is_preview = preview.is_some();
        let profile = preview.map(Personality::validated).transpose()?;
        let (settings, profile, pairs, message_id) = self.db(|s| {
            let settings = s.settings()?;
            if !settings.enabled {
                return Err("请先启用本地聊天。".into());
            }
            let profile = match profile {
                Some(p) => p,
                None => s.profile()?,
            };
            let pairs = if is_preview {
                Vec::new()
            } else {
                s.context_pairs(now())?
            };
            let message_id = if is_preview {
                None
            } else {
                Some(s.begin(&id, &text, now())?)
            };
            Ok((settings, profile, pairs, message_id))
        })?;
        let cancelled = Arc::new(AtomicBool::new(false));
        *gate = Some(Active {
            id: id.clone(),
            cancelled: cancelled.clone(),
            message_id,
            preview: is_preview,
            partial: String::new(),
        });
        self.generation.fetch_add(1, Ordering::SeqCst);
        if app
            .get_webview_window("chat")
            .is_none_or(|w| !w.is_visible().unwrap_or(false))
        {
            cancelled.store(true, Ordering::SeqCst);
        }
        drop(gate);
        let mut messages = vec![json!({"role":"system","content":profile.system_prompt()})];
        messages.extend(pairs);
        messages.push(json!({"role":"user","content":text}));
        let service = self.clone();
        let worker_id = id.clone();
        if std::thread::Builder::new()
            .name("pet-ai-reply".into())
            .spawn(move || {
                service.work(
                    app,
                    worker_id,
                    settings,
                    messages,
                    profile.output_tokens(),
                    cancelled,
                )
            })
            .is_err()
        {
            let mut gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
            if gate.as_ref().is_some_and(|a| a.id == id) {
                if let Some(mid) = gate.take().and_then(|a| a.message_id) {
                    self.db(|s| s.finish(mid, "", "failed"))?;
                }
            }
            return Err("无法启动回复任务，请稍后重试。".into());
        }
        Ok(())
    }
    fn work(
        &self,
        app: AppHandle,
        id: String,
        settings: Settings,
        messages: Vec<Value>,
        output: usize,
        cancelled: Arc<AtomicBool>,
    ) {
        let mut checkpoint = Instant::now();
        let result = self
            .runtime
            .ensure(&settings, &cancelled)
            .and_then(|endpoint| {
                ai_runtime::generate(&endpoint, messages, output, &cancelled, |part| {
                    let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
                    let a = guard
                        .as_mut()
                        .filter(|a| a.id == id && !a.cancelled.load(Ordering::SeqCst))
                        .ok_or("生成已停止。")?;
                    a.partial.push_str(part);
                    if checkpoint.elapsed() >= Duration::from_millis(500) {
                        if let Some(mid) = a.message_id {
                            self.db(|s| s.checkpoint(mid, &a.partial))?;
                        }
                        checkpoint = Instant::now();
                    }
                    let _ = app.emit_to(
                        "chat",
                        "ai:reply",
                        Event {
                            request_id: id.clone(),
                            content: a.partial.clone(),
                            status: "generating".into(),
                            preview: a.preview,
                            error: None,
                        },
                    );
                    Ok(())
                })
            });
        if result.is_err() {
            self.runtime.stop();
        }
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().is_none_or(|a| a.id != id) {
            return;
        }
        let a = guard.take().expect("matching request");
        let status = if a.cancelled.load(Ordering::SeqCst) {
            "cancelled"
        } else if result.is_ok() {
            "complete"
        } else {
            "failed"
        };
        let mut error = if status == "failed" {
            result.err()
        } else {
            None
        };
        if let Some(mid) = a.message_id {
            if let Err(e) = self.db(|s| s.finish(mid, &a.partial, status)) {
                error = Some(e);
            }
        }
        self.runtime.touch();
        let _ = app.emit_to(
            "chat",
            "ai:reply",
            Event {
                request_id: id,
                content: a.partial,
                status: if error.is_some() {
                    "failed".into()
                } else {
                    status.into()
                },
                preview: a.preview,
                error,
            },
        );
    }
    fn clear(&self) -> Result<()> {
        let gate = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if gate.is_some() {
            return Err("请先停止回复，再清空聊天。".into());
        }
        self.db(Store::clear)
    }
}
fn chat_only(window: &WebviewWindow) -> Result<()> {
    if window.label() != "chat" {
        Err("请在聊天窗口执行此操作。".into())
    } else {
        Ok(())
    }
}
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| "AI后台任务未完成，请重试。".to_string())?
}
#[tauri::command]
pub async fn ai_snapshot(window: WebviewWindow, state: State<'_, AiState>) -> Result<Snapshot> {
    chat_only(&window)?;
    let s = state.0.clone();
    blocking(move || s.snapshot()).await
}
#[tauri::command]
pub async fn ai_save_personality(
    window: WebviewWindow,
    state: State<'_, AiState>,
    personality: Personality,
) -> Result<Personality> {
    chat_only(&window)?;
    let s = state.0.clone();
    blocking(move || s.save_profile(personality)).await
}
#[tauri::command]
pub async fn ai_configure(
    window: WebviewWindow,
    state: State<'_, AiState>,
    settings: Settings,
) -> Result<()> {
    chat_only(&window)?;
    let s = state.0.clone();
    blocking(move || s.configure(settings)).await
}
#[tauri::command]
pub async fn ai_send(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, AiState>,
    request_id: String,
    text: String,
    preview: Option<Personality>,
) -> Result<()> {
    chat_only(&window)?;
    let s = state.0.clone();
    blocking(move || s.start(app, request_id, text, preview)).await
}
#[tauri::command]
pub async fn ai_cancel(
    window: WebviewWindow,
    state: State<'_, AiState>,
    request_id: String,
) -> Result<()> {
    chat_only(&window)?;
    let s = state.0.clone();
    blocking(move || {
        s.cancel_request(&request_id);
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn ai_clear_chat(window: WebviewWindow, state: State<'_, AiState>) -> Result<()> {
    chat_only(&window)?;
    let s = state.0.clone();
    blocking(move || s.clear()).await
}
#[tauri::command]
pub fn ai_ready(window: WebviewWindow) -> Result<()> {
    chat_only(&window)?;
    update_focus_layer(window.app_handle(), true);
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    window
        .show()
        .and_then(|_| window.set_focus())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn close_chat(window: WebviewWindow, state: State<'_, AiState>) -> Result<()> {
    chat_only(&window)?;
    let generation = state.0.generation();
    window.hide().map_err(|e| e.to_string())?;
    update_focus_layer(window.app_handle(), false);
    let s = state.0.clone();
    blocking(move || {
        s.cancel_generation(generation);
        Ok(())
    })
    .await
}
#[tauri::command]
pub fn open_chat(app: AppHandle) -> Result<()> {
    open_chat_window(&app).map_err(|e| e.to_string())
}
pub fn open_chat_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("chat") {
        update_focus_layer(app, true);
        window.set_always_on_top(true)?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        app,
        "chat",
        tauri::WebviewUrl::App("index.html?view=chat".into()),
    )
    .title("和大熊聊聊")
    .inner_size(500.0, 680.0)
    .min_inner_size(380.0, 480.0)
    .always_on_top(true)
    .center()
    .visible(false)
    .build()?;
    Ok(())
}

fn update_focus_layer(app: &AppHandle, focused: bool) {
    app.state::<AiState>()
        .0
        .chat_focused
        .store(focused, Ordering::Relaxed);
    if let Some(pet) = app.get_webview_window("main") {
        // Two always-on-top windows can trade order as the pet moves. Put the pet
        // in the normal layer while chatting, so it cannot intercept form input.
        let _ = pet.set_always_on_top(!focused);
    }
    if let Some(chat) = app.get_webview_window("chat") {
        let _ = chat.set_always_on_top(focused);
    }
    update_chat_hitbox(app);
}

fn update_chat_hitbox(app: &AppHandle) {
    let state = app.state::<AiState>();
    let rect = if state.0.chat_focused.load(Ordering::Relaxed) {
        app.get_webview_window("chat").and_then(|chat| {
            if !chat.is_visible().unwrap_or(false) {
                return None;
            }
            let position = chat.outer_position().ok()?;
            let size = chat.outer_size().ok()?;
            Some((position.x, position.y, size.width, size.height))
        })
    } else {
        None
    };
    *state
        .0
        .chat_hitbox
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = rect;
}

pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::Focused(focused) => update_focus_layer(window.app_handle(), *focused),
        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
            update_chat_hitbox(window.app_handle())
        }
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let service = window.state::<AiState>().0.clone();
            let generation = service.generation();
            let _ = window.hide();
            update_focus_layer(window.app_handle(), false);
            tauri::async_runtime::spawn_blocking(move || service.cancel_generation(generation));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn service(path: PathBuf) -> Arc<Service> {
        Service::new(
            path,
            Assets {
                root: PathBuf::from("missing-test-resources"),
            },
        )
    }
    #[test]
    fn stale_stop_and_close_do_not_cancel_a_newer_request() {
        let dir = tempfile::tempdir().unwrap();
        let s = service(dir.path().join("db"));
        let flag = Arc::new(AtomicBool::new(false));
        *s.active.lock().unwrap() = Some(Active {
            id: "new".into(),
            cancelled: flag.clone(),
            message_id: None,
            preview: true,
            partial: "试聊".into(),
        });
        s.generation.store(2, Ordering::SeqCst);
        s.cancel_request("old");
        s.cancel_generation(1);
        assert!(!flag.load(Ordering::SeqCst));
        s.cancel_request("new");
        assert!(flag.load(Ordering::SeqCst));
        s.shutdown();
    }
    #[test]
    fn shutdown_preserves_partial_reply_and_busy_clear_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let s = service(dir.path().join("db"));
        let id = s.db(|db| db.begin("one", "问题", now())).unwrap();
        *s.active.lock().unwrap() = Some(Active {
            id: "one".into(),
            cancelled: Arc::new(AtomicBool::new(false)),
            message_id: Some(id),
            preview: false,
            partial: "收到的回复".into(),
        });
        assert!(s.clear().is_err());
        assert!(s
            .configure(Settings {
                enabled: true,
                threads: 4
            })
            .is_err());
        s.shutdown();
        let snapshot = s.snapshot().unwrap();
        assert!(snapshot.active_request_id.is_none());
        assert_eq!(snapshot.messages[1].content, "收到的回复");
        assert_eq!(snapshot.messages[1].status, "cancelled");
    }
}
