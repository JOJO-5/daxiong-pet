// 桌面宠物不需要控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod atlas;
mod config;
mod engine;
mod petpack;
mod platform;
mod tray;

use config::Config;
use engine::{Command as EngineCommand, Engine, Input};
use petpack::PetPack;
use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow};

/// 把睡眠帧打包成一个整数，方便跨线程无锁传递：高 8 位是行号，低 8 位是列号
fn pack_sleep(row: u8, col: usize) -> u16 {
    ((row as u16) << 8) | (col as u16 & 0xFF)
}

fn unpack_sleep(v: u16) -> (u8, usize) {
    ((v >> 8) as u8, (v & 0xFF) as usize)
}

#[derive(Clone, serde::Serialize)]
struct FramePayload {
    row: u8,
    col: usize,
}

/// 宠物状态变化（目前只有睡眠），前端据此加视觉效果
#[derive(Clone, serde::Serialize)]
struct StatePayload {
    sleeping: bool,
}

/// 下发给前端的"当前宠物"
#[derive(Clone, serde::Serialize)]
struct PetSwitchPayload {
    id: String,
    name: String,
    rows: u32,
    /// 图集的 data URL；内置宠物为 None，前端回落到自身打包的资源
    data_url: Option<String>,
    /// 宠物包自带的 speech.json（可选）：有就覆盖前端的默认话术
    #[serde(skip_serializing_if = "Option::is_none")]
    speech: Option<serde_json::Value>,
}

/// 跨线程共享的应用状态
struct AppState {
    pets: Mutex<Vec<PetPack>>,
    current: Mutex<String>,
    /// 当前宠物是否带注视行，引擎每 tick 读一次
    look_enabled: Arc<AtomicBool>,
    /// 重力开关
    gravity: Arc<AtomicBool>,
    /// 当前宠物的睡眠帧（打包成 row<<8 | col），引擎每 tick 读一次
    sleep_frame: Arc<AtomicU16>,
    /// 给引擎线程发指令（番茄钟等）
    engine_tx: Mutex<mpsc::Sender<EngineCommand>>,
    pet_revision: Arc<AtomicU64>,
}

/// 前端完成精灵图解码后调用，此时才真正显示窗口，避免透明窗口白闪。
#[tauri::command]
fn pet_ready(window: WebviewWindow) {
    let _ = window.show();
}

/// 启动时前端来取"该显示哪只宠物"
#[tauri::command]
fn current_pet(app: AppHandle) -> Result<PetSwitchPayload, String> {
    let state = app.state::<AppState>();
    let id = state.current.lock().unwrap().clone();
    payload_for(state.inner(), &id).map_err(|e| e.to_string())
}

/// 列出所有可用宠物
#[tauri::command]
fn list_pets(state: State<'_, AppState>) -> Vec<PetPack> {
    state.pets.lock().unwrap().clone()
}

/// 切换宠物
#[tauri::command]
fn set_pet(id: String, app: AppHandle) -> Result<(), String> {
    switch_to_pet(&app, &id).map_err(|e| e.to_string())
}

/// 重新扫描宠物目录
#[tauri::command]
fn rescan_pets(app: AppHandle) -> Result<(), String> {
    rescan_and_refresh(&app).map_err(|e| e.to_string())
}

/// 开关重力
#[tauri::command]
fn set_gravity(enabled: bool, app: AppHandle) -> bool {
    apply_gravity(&app, enabled)
}

/// 开始 / 取消番茄钟（25 分钟）
#[tauri::command]
fn set_pomodoro(active: bool, app: AppHandle) {
    let state = app.state::<AppState>();
    let tx = state.engine_tx.lock().unwrap();
    let cmd = if active {
        EngineCommand::StartPomodoro
    } else {
        EngineCommand::CancelPomodoro
    };
    let _ = tx.send(cmd);
}

/// 重力是纯开关：落盘后由引擎线程每 tick 读取
pub(crate) fn apply_gravity(app: &AppHandle, enabled: bool) -> bool {
    let state = app.state::<AppState>();
    state.gravity.store(enabled, Ordering::Relaxed);

    let pet_id = state.current.lock().unwrap().clone();
    Config {
        pet_id: Some(pet_id),
        gravity: enabled,
    }
    .save(app);

    enabled
}

/// 组装下发给前端的宠物信息（含图集 data URL）
fn payload_for(state: &AppState, id: &str) -> std::io::Result<PetSwitchPayload> {
    let pack = {
        let pets = state.pets.lock().unwrap();
        pets.iter().find(|p| p.id == id).cloned()
    }
    .unwrap_or_else(petpack::builtin);

    // 内置宠物的 sheet 是空路径，前端会用自己的资源
    let data_url = if pack.sheet.as_os_str().is_empty() {
        None
    } else {
        Some(petpack::sheet_data_url(&pack)?)
    };

    // 宠物包自带话术（可选增强）：跟图集同目录的 speech.json
    let speech = pack
        .sheet
        .parent()
        .map(|dir| dir.join("speech.json"))
        .filter(|p| p.is_file())
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());

    Ok(PetSwitchPayload {
        id: pack.id,
        name: pack.name,
        rows: pack.rows,
        data_url,
        speech,
    })
}

/// 切换宠物：更新状态、持久化、通知前端、刷新托盘菜单
pub(crate) fn switch_to_pet(app: &AppHandle, id: &str) -> std::io::Result<()> {
    let state = app.state::<AppState>();

    let pack = {
        let pets = state.pets.lock().unwrap();
        pets.iter().find(|p| p.id == id).cloned()
    };
    let Some(pack) = pack else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "宠物不存在",
        ));
    };

    // 先成功读取图片再切状态，读取失败保留原宠物。
    let payload = payload_for(state.inner(), &pack.id)?;
    *state.current.lock().unwrap() = pack.id.clone();
    state.look_enabled.store(pack.has_look(), Ordering::Relaxed);
    state
        .sleep_frame
        .store(pack_sleep(pack.sleep_row, pack.sleep_col), Ordering::Relaxed);

    Config {
        pet_id: Some(pack.id.clone()),
        gravity: state.gravity.load(Ordering::Relaxed),
    }
    .save(app);

    let _ = app.emit("pet:switch", payload);
    state.pet_revision.fetch_add(1, Ordering::Release);

    let pets = state.pets.lock().unwrap().clone();
    let gravity = state.gravity.load(Ordering::Relaxed);
    tray::refresh(app, &pets, &pack.id, gravity);

    Ok(())
}

/// 重新扫描宠物目录；当前宠物若已消失就自动回落到第一只可用的
pub(crate) fn rescan_and_refresh(app: &AppHandle) -> std::io::Result<()> {
    let state = app.state::<AppState>();

    let found = petpack::discover();
    let current = state.current.lock().unwrap().clone();
    let selected = if found.iter().any(|p| p.id == current) { current } else { "__builtin__".into() };
    *state.pets.lock().unwrap() = found;
    // 当前 ID 未变也重新读图和话术，同步注视、睡眠帧及托盘。
    switch_to_pet(app, &selected)
}

/// 取当前所在显示器的工作区（物理像素）。
fn screen_rect(window: &WebviewWindow) -> (i32, i32, i32, i32) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let area = monitor.work_area();
        let pos = &area.position;
        let size = &area.size;
        return (pos.x, pos.y, size.width as i32, size.height as i32);
    }
    // 兜底：拿不到显示器信息时按 1080p 处理，总比 panic 好
    (0, 0, 1920, 1080)
}

fn spawn_engine(
    app: AppHandle,
    window: WebviewWindow,
    look_enabled: Arc<AtomicBool>,
    gravity: Arc<AtomicBool>,
    sleep_frame: Arc<AtomicU16>,
    cmd_rx: mpsc::Receiver<EngineCommand>,
    pet_revision: Arc<AtomicU64>,
) {
    std::thread::spawn(move || {
        let mut engine = Engine::new();
        engine.attach_commands(cmd_rx);

        let mut last = Instant::now();
        let mut prev_frame: Option<(u8, usize)> = None;
        let mut prev_clickable: Option<bool> = None;
        let mut prev_sleeping: Option<bool> = None;
        let mut screen = screen_rect(&window);
        let mut ticks: u64 = 0;
        let mut revision = pet_revision.load(Ordering::Acquire);

        loop {
            // 目标 60Hz。桌宠不需要更高，再高只是白烧 CPU。
            std::thread::sleep(Duration::from_millis(16));

            let now = Instant::now();
            let dt = (now - last).as_millis().min(u64::MAX as u128) as u64;
            last = now;
            ticks += 1;

            // 显示器可能被热插拔或改变分辨率，定期刷新边界
            if ticks % 60 == 0 {
                screen = screen_rect(&window);
            }

            let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
                continue;
            };

            let (cursor, button_down) = platform::pointer_state(&window);
            let input = Input {
                dt_ms: dt,
                cursor,
                win_pos: (pos.x, pos.y),
                win_size: (size.width as i32, size.height as i32),
                scale_factor: window.scale_factor().unwrap_or(1.0),
                screen,
                button_down,
                look_enabled: look_enabled.load(Ordering::Relaxed),
                gravity: gravity.load(Ordering::Relaxed),
                local_hour: platform::local_hour(),
                sleep_frame: {
                    let (row, col) = unpack_sleep(sleep_frame.load(Ordering::Relaxed));
                    (
                        atlas::Row::from_index(row).unwrap_or(atlas::Row::Failed),
                        col,
                    )
                },
            };

            let current_revision = pet_revision.load(Ordering::Acquire);
            if revision != current_revision {
                prev_frame = None;
                prev_sleeping = None;
                revision = current_revision;
            }
            let out = engine.tick(&input);

            if let Some((x, y)) = out.move_to {
                let _ = window.set_position(PhysicalPosition::new(x, y));
            }

            // 穿透状态只在变化时下发，避免每帧都过一遍 IPC
            if prev_clickable != Some(out.clickable) {
                let _ = window.set_ignore_cursor_events(!out.clickable);
                prev_clickable = Some(out.clickable);
            }

            let frame = (out.row, out.col);
            if prev_frame != Some(frame) {
                let _ = app.emit("pet:frame", FramePayload { row: frame.0, col: frame.1 });
                prev_frame = Some(frame);
            }

            if prev_sleeping != Some(out.sleeping) {
                let _ = app.emit("pet:state", StatePayload { sleeping: out.sleeping });
                prev_sleeping = Some(out.sleeping);
            }

            // 说话是离散事件，每 tick 至多一次，不需要去重
            if let Some(kind) = out.say {
                let _ = app.emit("pet:say", kind.as_str());
            }
        }
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            pet_ready,
            current_pet,
            list_pets,
            set_pet,
            rescan_pets,
            set_gravity,
            set_pomodoro
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").expect("缺少 main 窗口");

            // 启动瞬间先完全穿透，绝不抢桌面点击
            let _ = window.set_ignore_cursor_events(true);

            let saved = Config::load(app.handle());

            // 扫描宠物包；一个都没有时用内置的兜底
            let mut pets = petpack::discover();
            if pets.is_empty() {
                pets.push(petpack::builtin());
            }

            // 初始选中：配置里记住的 > 第一只 > 内置
            let initial = saved
                .pet_id
                .clone()
                .filter(|id| pets.iter().any(|p| p.id == *id))
                .unwrap_or_else(|| pets[0].id.clone());

            let look_enabled = Arc::new(AtomicBool::new(
                pets.iter()
                    .find(|p| p.id == initial)
                    .map(|p| p.has_look())
                    .unwrap_or(true),
            ));
            let gravity = Arc::new(AtomicBool::new(saved.gravity));

            // 睡眠帧随宠物走：契约里没有睡觉动画，默认借 failed 行第 2 格（趴着侧躺）
            let initial_pack = pets.iter().find(|p| p.id == initial);
            let sleep_frame = Arc::new(AtomicU16::new(pack_sleep(
                initial_pack
                    .map(|p| p.sleep_row)
                    .unwrap_or(petpack::DEFAULT_SLEEP_ROW),
                initial_pack
                    .map(|p| p.sleep_col)
                    .unwrap_or(petpack::DEFAULT_SLEEP_COL),
            )));

            let (tx, rx) = mpsc::channel();
            let pet_revision = Arc::new(AtomicU64::new(0));

            app.manage(AppState {
                pets: Mutex::new(pets.clone()),
                current: Mutex::new(initial.clone()),
                look_enabled: look_enabled.clone(),
                gravity: gravity.clone(),
                sleep_frame: sleep_frame.clone(),
                engine_tx: Mutex::new(tx),
                pet_revision: pet_revision.clone(),
            });

            tray::build(app.handle(), &pets, &initial, saved.gravity)?;

            // 兜底：前端若因故没发出 ready，3 秒后也强制显示，不留一个隐形进程
            let fallback = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(3000));
                if let Some(w) = fallback.get_webview_window("main") {
                    let _ = w.show();
                }
            });

            spawn_engine(
                app.handle().clone(),
                window,
                look_enabled,
                gravity,
                sleep_frame,
                rx,
                pet_revision,
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
