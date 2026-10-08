// 桌面宠物不需要控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod atlas;
mod activities;
mod config;
mod companion;
mod engine;
mod petting;
mod encounters;
mod dizziness;
mod petpack;
mod play;
mod tug;
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
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow};

/// 把睡眠帧打包成一个整数，方便跨线程无锁传递：高 8 位是行号，低 8 位是列号
fn pack_sleep(row: u8, col: usize) -> u16 {
    ((row as u16) << 8) | (col as u16 & 0xFF)
}

fn unpack_sleep(v: u16) -> (u8, usize) {
    ((v >> 8) as u8, (v & 0xFF) as usize)
}

#[derive(Clone, PartialEq, serde::Serialize)]
struct ReleasedToy {
    toy: &'static str,
    x: f64,
    y: f64,
}

#[derive(Clone, PartialEq, serde::Serialize)]
struct FramePayload {
    row: u8,
    col: usize,
    released_toy: Option<ReleasedToy>,
    petting: Option<petting::Feedback>,
    dizzy: Option<dizziness::Feedback>,
}

/// 宠物状态变化（目前只有睡眠），前端据此加视觉效果
#[derive(Clone, serde::Serialize)]
struct StatePayload {
    sleeping: bool,
    clickable: bool,
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
    requested_visible: AtomicBool,
    focusing: AtomicBool,
    play: Mutex<play::PlayView>,
    memory: Mutex<MemoryState>,
    activity: Mutex<activities::ActivityView>,
    rewarded_trick: AtomicU64,
    encounter: Mutex<encounters::EncounterView>,
    shortcut: Mutex<ShortcutPrefs>,
    pet_menu:Mutex<Option<tauri::menu::Menu<tauri::Wry>>>,
}

#[derive(Clone,Default)]
struct ShortcutPrefs {requested:bool,error:Option<String>}
#[derive(serde::Serialize)]
struct ShortcutView {requested:bool,enabled:bool,key:&'static str,error:Option<String>}
const OPEN_SHORTCUT:&str="Ctrl+Alt+P";

struct MemoryState { data: companion::Memory, error: Option<String> }

fn memory_path(app: &AppHandle) -> Result<std::path::PathBuf,String> {
    app.path().app_config_dir().map(|p|p.join("companion.json")).map_err(|e|e.to_string())
}

#[tauri::command]
fn companion_status(app: AppHandle) -> companion::MemoryView {
    let state=app.state::<AppState>();let m=state.memory.lock().unwrap();
    m.data.view(companion::now(),m.error.clone())
}

fn update_memory(app:&AppHandle, change:impl FnOnce(&mut companion::Memory)->Result<bool,String>) -> Result<companion::MemoryView,String> {
    let state=app.state::<AppState>();let mut stored=state.memory.lock().unwrap();
    if let Some(error)=&stored.error { return Err(format!("{error}；请先在互动面板恢复记忆")); }
    let mut next=stored.data.clone();
    if change(&mut next)? {
        next.save(&memory_path(app)?).map_err(|e|format!("保存陪伴记忆失败：{e}"))?;
        stored.data=next;
    }
    let view=stored.data.view(companion::now(),None);
    let _=app.emit("pet:memory",&view);
    Ok(view)
}

#[tauri::command]
fn set_nickname(nickname:String,app:AppHandle) -> Result<companion::MemoryView,String> {
    let nickname=companion::Memory::validated_nickname(&nickname)?;
    update_memory(&app,|m|{m.nickname=nickname;Ok(true)})
}

#[tauri::command]
fn feed_treat(app:AppHandle) -> Result<companion::MemoryView,String> {
    let state=app.state::<AppState>();
    if !state.requested_visible.load(Ordering::Relaxed) { return Err("先显示大熊再喂饼干吧".into()); }
    let view=update_memory(&app,|m|m.reward(companion::Reward::Treat,companion::now()))?;
    state.engine_tx.lock().unwrap().send(EngineCommand::FeedTreat).map_err(|e|e.to_string())?;
    let name=if view.nickname.is_empty() { "你" } else { &view.nickname };
    let _=app.emit("pet:message",format!("谢谢{name}！这块饼干真香。"));
    let _=app.emit("pet:treat",());
    Ok(view)
}

#[tauri::command]
fn set_encounters(enabled:bool,app:AppHandle)->Result<companion::MemoryView,String> {
    update_memory(&app,|m|{m.encounters_enabled=enabled;Ok(true)})
}

#[tauri::command]
fn encounter_status(app:AppHandle)->serde_json::Value {
    let state=app.state::<AppState>();let memory=state.memory.lock().unwrap();let event=state.encounter.lock().unwrap();
    serde_json::json!({"enabled":memory.data.encounters_enabled,"quiet_companion":memory.data.quiet_companion,"kind":event.kind,"phase":event.phase,"right":event.right})
}

#[tauri::command]
fn set_playful_fetch(enabled:bool,app:AppHandle)->Result<companion::MemoryView,String> {
    update_memory(&app,|m|{m.playful_fetch=enabled;Ok(true)})
}

#[tauri::command]
fn set_quiet_companion(enabled:bool,app:AppHandle)->Result<companion::MemoryView,String> {
    update_memory(&app,|m|{m.quiet_companion=enabled;Ok(true)})
}

#[tauri::command]
fn memory_recovery_info(app:AppHandle)->Result<serde_json::Value,String> {
    let path=memory_path(&app)?;
    Ok(serde_json::json!({"directory":path.parent().map(|p|p.to_string_lossy().into_owned()),"has_file":path.is_file()}))
}

#[tauri::command]
fn restore_memory(app:AppHandle) -> Result<companion::MemoryView,String> {
    let state=app.state::<AppState>();let mut stored=state.memory.lock().unwrap();
    if stored.error.is_none() { return Err("记忆正常，无需恢复".into()); }
    let path=memory_path(&app)?;
    if path.is_file() {
        let backup=path.with_file_name(format!("companion.backup-{}.json",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
        std::fs::copy(&path,backup).map_err(|e|format!("备份记忆失败：{e}"))?;
    }
    let next=companion::Memory::default();next.save(&path).map_err(|e|e.to_string())?;
    stored.data=next;stored.error=None;
    let view=stored.data.view(companion::now(),None);let _=app.emit("pet:memory",&view);
    Ok(view)
}

/// 前端完成精灵图解码后调用，此时才真正显示窗口，避免透明窗口白闪。
#[tauri::command]
fn pet_ready(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    if app.state::<AppState>().requested_visible.load(Ordering::Relaxed) {
        window.show().map_err(|e| e.to_string())?;
    }
    Ok(())
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
fn set_gravity(enabled: bool, app: AppHandle) -> Result<bool, String> {
    apply_gravity(&app, enabled).map_err(|e| e.to_string())
}

/// 开始 / 取消番茄钟（25 分钟）
#[tauri::command]
fn set_pomodoro(active: bool, app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let tx = state.engine_tx.lock().unwrap();
    let cmd = if active {
        EngineCommand::StartPomodoro
    } else {
        EngineCommand::CancelPomodoro
    };
    tx.send(cmd).map_err(|e| e.to_string())
}


#[tauri::command]
fn set_visible(visible: bool, app: AppHandle) -> Result<(), String> {
    apply_visibility(&app,visible).map_err(|e|e.to_string())
}

pub(crate) fn apply_visibility(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    if let Some(window)=app.get_webview_window("main") {
        if visible { window.show()?; } else { window.hide()?; }
        app.state::<AppState>().requested_visible.store(visible,Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
fn snack_action(action:String,difficulty:Option<String>,app:AppHandle)->Result<(),String> {
    let state=app.state::<AppState>();
    if !state.requested_visible.load(Ordering::Relaxed) && action!="cancel" {return Err("先显示大熊再找零食吧".into());}
    let cmd=match action.as_str() {
        "place"=>{let far=match difficulty.as_deref().unwrap_or("easy") {"easy"=>false,"far"=>true,_=>return Err("未知难度".into())};EngineCommand::PlaceSnack(far)},
        "find"=>EngineCommand::FindSnack,"cancel"=>EngineCommand::CancelPlay,_=>return Err("未知零食游戏操作".into())
    };
    state.engine_tx.lock().unwrap().send(cmd).map_err(|e|e.to_string())?;if action!="cancel" {let _=app.emit_to("playground","pet:game","snack");}Ok(())
}
#[tauri::command]
fn trick_status(app:AppHandle)->activities::ActivityView {app.state::<AppState>().activity.lock().unwrap().clone()}
#[tauri::command]
fn trick_action(cue:String,app:AppHandle)->Result<(),String> {
    let state=app.state::<AppState>();
    if !state.requested_visible.load(Ordering::Relaxed) {return Err("先显示大熊再练习吧".into());}
    let cmd=if cue=="stop" {EngineCommand::CancelPlay} else {
        let cue=activities::Cue::parse(&cue)?;
        let learned=state.memory.lock().unwrap().data.training[cue.index()]>=3;
        EngineCommand::Trick(cue,learned)
    };
    state.engine_tx.lock().unwrap().send(cmd).map_err(|e|e.to_string())?;if cue!="stop" {let _=app.emit_to("playground","pet:game","tricks");}Ok(())
}
#[tauri::command]
fn reward_trick(app:AppHandle)->Result<companion::MemoryView,String> {
    let state=app.state::<AppState>();let activity=state.activity.lock().unwrap();
    if !activity.rewardable || activity.id==state.rewarded_trick.load(Ordering::Acquire) {return Err("先完成一个小指令再奖励吧".into());}
    let cue=activity.kind.ok_or("没有可奖励的指令")?;
    let view=update_memory(&app,|m|m.reward_trick(cue,companion::now()))?;
    state.rewarded_trick.store(activity.id,Ordering::Release);
    state.engine_tx.lock().unwrap().send(EngineCommand::FeedTreat).map_err(|e|e.to_string())?;
    let _=app.emit("pet:treat",());let _=app.emit("pet:message",format!("谢谢{}！我记住‘{}’啦。",if view.nickname.is_empty(){"你"}else{&view.nickname},cue.label()));
    Ok(view)
}

#[tauri::command]
fn play_status(app: AppHandle) -> play::PlayView { app.state::<AppState>().play.lock().unwrap().clone() }

#[tauri::command]
fn play_action(action: String, app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.requested_visible.load(Ordering::Relaxed) && action != "cancel" { return Err("先显示大熊再一起玩吧".into()); }
    if action!="cancel" && state.focusing.load(Ordering::Relaxed) {return Err("正在专注，结束专注后再玩吧".into());}
    let cmd = match action.as_str() {
        "start_tug" => EngineCommand::StartTug,
        "show" => EngineCommand::ShowBall, "throw" => EngineCommand::ThrowBall,
        "show_frisbee"=>EngineCommand::ShowFrisbee,"throw_frisbee"=>EngineCommand::ThrowFrisbee,
        "cancel" => EngineCommand::CancelPlay, "drop" => EngineCommand::DropBall, "roll" => EngineCommand::RollBall, _ => return Err("未知互动".into()),
    };
    state.engine_tx.lock().unwrap().send(cmd).map_err(|e| e.to_string())?;
    if matches!(action.as_str(),"start_tug"|"show"|"throw"|"show_frisbee"|"throw_frisbee") {let _=app.emit_to("playground","pet:game",if action=="start_tug" {"tug"} else if action.contains("frisbee"){"frisbee"}else{"fetch"});}
    Ok(())
}

#[tauri::command]
fn shortcut_status(app:AppHandle)->ShortcutView {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let state=app.state::<AppState>();let prefs=state.shortcut.lock().unwrap();
    ShortcutView {requested:prefs.requested,enabled:app.global_shortcut().is_registered(OPEN_SHORTCUT),key:OPEN_SHORTCUT,error:prefs.error.clone()}
}
#[tauri::command]
fn set_shortcut(enabled:bool,app:AppHandle)->Result<ShortcutView,String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let state=app.state::<AppState>();let mut prefs=state.shortcut.lock().unwrap();
    let was_registered=app.global_shortcut().is_registered(OPEN_SHORTCUT);
    let change=if enabled && !was_registered {app.global_shortcut().register(OPEN_SHORTCUT)}
        else if !enabled && was_registered {app.global_shortcut().unregister(OPEN_SHORTCUT)} else {Ok(())};
    if let Err(e)=change {let text=format!("快捷键未生效：{e}，可以继续用右键菜单或托盘打开");prefs.error=Some(text.clone());return Err(text);}
    let saved=(||{let mut config=Config::load(&app)?;config.shortcut_enabled=enabled;config.save(&app)})();
    if let Err(e)=saved {
        let rollback=if was_registered && !enabled {app.global_shortcut().register(OPEN_SHORTCUT)}
            else if !was_registered && enabled {app.global_shortcut().unregister(OPEN_SHORTCUT)} else {Ok(())};
        let text=match rollback {Ok(())=>format!("快捷键设置未保存：{e}"),Err(r)=>format!("设置未保存：{e}；恢复快捷键失败：{r}")};
        prefs.error=Some(text.clone());return Err(text);
    }
    prefs.requested=enabled;prefs.error=None;drop(prefs);Ok(shortcut_status(app))
}
#[tauri::command]
fn close_playground(app:AppHandle)->Result<(),String> {
    if let Some(window)=app.get_webview_window("playground") {window.hide().map_err(|e|e.to_string())?;}Ok(())
}
#[tauri::command]
fn open_pet_menu(app:AppHandle)->Result<(),String> {
    use tauri::menu::{ContextMenu,MenuBuilder,MenuItem,SubmenuBuilder};
    let window=app.get_webview_window("main").ok_or("大熊窗口未打开")?;
    let view=companion_status(app.clone());
    let state=app.state::<AppState>();
    let play=state.play.lock().unwrap().clone();
    let activity=state.activity.lock().unwrap().clone();
    let interacting=play.phase!="off" || !matches!(activity.phase,"off"|"blocked"|"cancelled"|"found");
    let label=if view.treat_wait>0 {format!("喂饼干（{} 秒后）",view.treat_wait)} else if interacting {"结束互动，喂块饼干".into()} else {"喂一块饼干".into()};
    let feed=MenuItem::with_id(&app,"pet_feed",label,view.treat_wait==0 && view.error.is_none(),None::<&str>).map_err(|e|e.to_string())?;
    let more=SubmenuBuilder::new(&app,"更多").text("pet_preferences","记忆与偏好…").text("pomodoro","专注 / 结束专注").text("toggle","隐藏大熊").build().map_err(|e|e.to_string())?;
    let menu=MenuBuilder::new(&app).text("play","打开互动面板…").separator().item(&feed)
        .text("pet_ball","抛一球").text("pet_frisbee","扔飞盘").text("pet_tug","一起拔河").text("pet_come","过来")
        .text("pet_stop","收起玩具 / 结束练习").separator().item(&more).build().map_err(|e|e.to_string())?;
    *app.state::<AppState>().pet_menu.lock().unwrap()=Some(menu.clone());
    menu.popup(window.as_ref().window()).map_err(|e|e.to_string())?;
    let _=app.emit("pet:onboarding-complete",());Ok(())
}
fn position_panel(app:&AppHandle,panel:&WebviewWindow)->tauri::Result<()> {
    if let Some(pet)=app.get_webview_window("main") {
        let (sx,sy,sw,sh)=screen_rect(&pet);let pos=pet.outer_position()?;let size=pet.outer_size()?;
        let panel_size=panel.outer_size()?;let gap=(12.0*pet.scale_factor()?).round() as i32;
        let right=pos.x+size.width as i32+gap;
        let x=if right+panel_size.width as i32<=sx+sw {right} else {pos.x-panel_size.width as i32-gap};
        panel.set_position(PhysicalPosition::new(x.clamp(sx,sx+(sw-panel_size.width as i32).max(0)),(pos.y-24).clamp(sy,sy+(sh-panel_size.height as i32).max(0))))?;
    }Ok(())
}
pub(crate) fn open_preferences(app:&AppHandle)->tauri::Result<()> {
    if app.get_webview_window("playground").is_some() {open_play_window(app)?;app.emit_to("playground","pet:preferences",())?;return Ok(());}
    tauri::WebviewWindowBuilder::new(app,"playground",tauri::WebviewUrl::App("index.html?view=playground&preferences=1".into()))
        .title("和大熊一起玩").inner_size(360.0,600.0).resizable(false).visible(false).build()?;Ok(())
}

pub(crate) fn open_help(app:&AppHandle)->tauri::Result<()> {
    if app.get_webview_window("playground").is_some() {open_play_window(app)?;app.emit_to("playground","pet:help",())?;return Ok(());}
    tauri::WebviewWindowBuilder::new(app,"playground",tauri::WebviewUrl::App("index.html?view=playground&help=1".into()))
        .title("和大熊一起玩").inner_size(360.0,600.0).resizable(false).visible(false).build()?;Ok(())
}

#[tauri::command]
fn playground_ready(window:WebviewWindow)->Result<(),String> {
    position_panel(window.app_handle(),&window).map_err(|e|e.to_string())?;
    window.show().and_then(|_|window.set_focus()).map_err(|e|e.to_string())?;
    let _=window.app_handle().emit("pet:onboarding-complete",());Ok(())
}

#[tauri::command]
fn open_playground(app: AppHandle) -> Result<(), String> { open_play_window(&app).map_err(|e|e.to_string()) }

pub(crate) fn open_play_window(app: &AppHandle) -> tauri::Result<()> {
    if !app.state::<AppState>().requested_visible.load(Ordering::Relaxed) {apply_visibility(app,true)?;}
    if let Some(window) = app.get_webview_window("playground") { position_panel(app,&window)?;window.show()?; window.set_focus()?; return Ok(()); }
    tauri::WebviewWindowBuilder::new(app,"playground",tauri::WebviewUrl::App("index.html?view=playground".into()))
        .title("和大熊一起玩").inner_size(360.0,600.0).resizable(false).visible(false).build()?;
    Ok(())
}

fn create_toy(app: &AppHandle) -> tauri::Result<()> {
    tauri::WebviewWindowBuilder::new(app,"toy",tauri::WebviewUrl::App("index.html?view=toy".into()))
        .title("大熊的球").inner_size(44.0,44.0).transparent(true).decorations(false)
        .resizable(false).always_on_top(true).skip_taskbar(true).shadow(false).focused(false).focusable(false)
        // The toy needs no shared browser storage. A separate context also keeps its hidden webview out of WebKit automation.
        .data_directory(app.path().app_cache_dir()?.join("toy-webview"))
        .visible(false).build()?;
    Ok(())
}

/// 重力是纯开关：落盘后由引擎线程每 tick 读取
pub(crate) fn apply_gravity(app: &AppHandle, enabled: bool) -> std::io::Result<bool> {
    let state = app.state::<AppState>();

    let pet_id = state.current.lock().unwrap().clone();
    Config {
        pet_id: Some(pet_id),
        gravity: enabled,
        shortcut_enabled:state.shortcut.lock().unwrap().requested,
    }
    .save(app)?;
    state.gravity.store(enabled, Ordering::Relaxed);
    Ok(enabled)
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

    let speech = if let Some(dir) = pack.sheet.parent() {
        match std::fs::read(dir.join("speech.json")) {
            Ok(bytes) => {
                let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
                if !value.is_object() { return Err(std::io::Error::other("speech.json 必须是话术对象")); }
                Some(value)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        }
    } else { None };

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
    Config {
        pet_id: Some(pack.id.clone()),
        gravity: state.gravity.load(Ordering::Relaxed),
        shortcut_enabled:state.shortcut.lock().unwrap().requested,
    }
    .save(app)?;

    *state.current.lock().unwrap() = pack.id.clone();
    state.look_enabled.store(pack.has_look(), Ordering::Relaxed);
    state
        .sleep_frame
        .store(pack_sleep(pack.sleep_row, pack.sleep_col), Ordering::Relaxed);

    app.emit("pet:switch", payload).map_err(std::io::Error::other)?;
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
        let mut prev_frame: Option<FramePayload> = None;
        let mut prev_clickable: Option<bool> = None;
        let mut prev_sleeping: Option<bool> = None;
        let mut prev_toy_hot:Option<bool>=None;
        let mut toy_logical_size=(44_i32,44_i32);
        let mut screen = screen_rect(&window);
        let mut last_screen_update = Instant::now();
        let mut visible = false;
        let mut input = Input {
            dt_ms: 0, interactive: false, cursor: (0, 0), win_pos: (0, 0),
            win_size: (engine::WINDOW_W, engine::WINDOW_H), scale_factor: 1.0,
            screen, button_down: false, look_enabled: true, extra_animations: false, gravity: false,
            encounters_enabled: true, local_hour: platform::local_hour(), sleep_frame: (atlas::Row::Failed, 2),
        };
        let mut revision = pet_revision.load(Ordering::Acquire);

        loop {
            // 目标 60Hz。桌宠不需要更高，再高只是白烧 CPU。
            std::thread::sleep(Duration::from_millis(engine::poll_interval_ms(visible, prev_sleeping.unwrap_or(false))));

            let now = Instant::now();
            let dt = (now - last).as_millis().min(u64::MAX as u128) as u64;
            last = now;
            let was_visible = visible;
            visible = window.is_visible().unwrap_or(false);
            input.dt_ms = dt;
            input.interactive = visible;
            input.local_hour = platform::local_hour();
            if visible {
                if !was_visible || last_screen_update.elapsed().as_secs() >= 1 {
                    screen = screen_rect(&window);
                    last_screen_update = now;
                }
                let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else { continue; };
                let (cursor, button_down) = platform::pointer_state(&window);
                input.cursor = cursor;
                input.button_down = button_down;
                input.win_pos = (pos.x, pos.y);
                input.win_size = (size.width as i32, size.height as i32);
                input.scale_factor = window.scale_factor().unwrap_or(1.0);
                input.screen = screen;
                if !was_visible { prev_frame = None; prev_sleeping = None; }
            } else {
                input.button_down = false;
            }
            {
                let state=app.state::<AppState>();let memory=state.memory.lock().unwrap();
                input.encounters_enabled=memory.data.encounters_enabled && !memory.data.quiet_companion;
                engine.set_quiet_companion(memory.data.quiet_companion);
            }
            input.look_enabled = look_enabled.load(Ordering::Relaxed);
            input.gravity = gravity.load(Ordering::Relaxed);
            input.extra_animations = app.state::<AppState>().current.lock().unwrap().as_str() == "__builtin__";
            let (row, col) = unpack_sleep(sleep_frame.load(Ordering::Relaxed));
            input.sleep_frame = (atlas::Row::from_index(row).unwrap_or(atlas::Row::Failed), col);

            let current_revision = pet_revision.load(Ordering::Acquire);
            if revision != current_revision {
                prev_frame = None;
                prev_sleeping = None;
                revision = current_revision;
                engine.cancel_play();
            }
            engine.set_playful_fetch(app.state::<AppState>().memory.lock().unwrap().data.playful_fetch);
            let out = engine.tick(&input);
            app.state::<AppState>().focusing.store(engine.is_focusing(),Ordering::Relaxed);

            if let Some((x, y)) = out.move_to {
                let _ = window.set_position(PhysicalPosition::new(x, y));
            }

            let mut activity=engine.activity_view();
            let state=app.state::<AppState>();
            if activity.id==state.rewarded_trick.load(Ordering::Acquire) {activity.rewardable=false;}
            let mut previous_activity=state.activity.lock().unwrap();
            if previous_activity.id!=activity.id || previous_activity.phase!=activity.phase || previous_activity.rewardable!=activity.rewardable || previous_activity.treat.is_some()!=activity.treat.is_some() {
                let _=app.emit("pet:activity",&activity);
                if activity.phase=="found" {
                    let _=app.emit("pet:message","找到啦！再藏一块让我找找？");let _=app.emit("pet:treat",());
                }
                if matches!(activity.phase,"attention" | "completed") {
                    if let Some(cue)=activity.kind {
                        let stored=state.memory.lock().unwrap();
                        let text=if activity.phase=="attention" {format!("{}，我先看看你，再做‘{}’。",stored.data.address(),cue.label())}
                            else {format!("{}，‘{}’做到了！可以给我一块奖励饼干吗？",stored.data.address(),cue.label())};
                        let _=app.emit("pet:message",text);
                    }
                }
            }
            let snack_position=activity.treat;
            *previous_activity=activity;drop(previous_activity);
            let play = engine.play_view();
            // The release pose and toy share one React update in the pet
            // webview. A separate native window can otherwise show a new toy
            // over the previous baked carry frame for a compositor frame.
            let released_toy = if input.extra_animations && play.phase=="releasing" {
                let position=out.move_to.unwrap_or(input.win_pos);
                play.ball.map(|(x,y)|ReleasedToy {toy:play.toy,
                    x:(x-position.0) as f64/input.scale_factor,
                    y:(y-position.1) as f64/input.scale_factor})
            } else {None};
            if let Some(toy) = app.get_webview_window("toy") {
                let visible_ball = snack_position.or_else(||if input.extra_animations && matches!(play.phase,"returning" | "teasing" | "catching" | "releasing") { None } else { play.ball });
                let toy_hot=visible_ball.is_some_and(|(x,y)| ((input.cursor.0-x) as f64).hypot((input.cursor.1-y) as f64)<=22.0*input.scale_factor);
                // GTK must realize the native window before applying its input shape.
                // Keep the transparent area click-through; only the actual toy accepts input.
                if toy.is_visible().unwrap_or(false) && prev_toy_hot!=Some(toy_hot) {let _=toy.set_ignore_cursor_events(!toy_hot);prev_toy_hot=Some(toy_hot);}
                if let Some((x,y)) = visible_ball {
                    let (center,desired)=if let Some(rope)=&play.tug {
                        // Keep the viewport stable while pulling. Native resize and WebKit
                        // resize events can otherwise render new endpoints in an old size,
                        // briefly detaching the rope from the baked bite tip. The maximum
                        // pull is 170 x 48 logical pixels, plus handle padding.
                        (((rope.mouth.0+rope.handle.0)/2,(rope.mouth.1+rope.handle.1)/2),(224,144))
                    } else {((x,y),(44,44))};
                    if desired!=toy_logical_size {
                        if toy.set_size(tauri::LogicalSize::new(desired.0 as f64,desired.1 as f64)).is_ok() {toy_logical_size=desired;}
                    }
                    let size = toy.outer_size().ok();
                    let half = size.map(|v| (v.width as i32/2,v.height as i32/2)).unwrap_or((22,22));
                    let _ = toy.set_position(PhysicalPosition::new(center.0-half.0,center.1-half.1));
                    if !toy.is_visible().unwrap_or(false) { let _ = toy.show(); }
                } else if toy.is_visible().unwrap_or(false) { let _ = toy.hide(); prev_toy_hot=None; }
            }
            let state = app.state::<AppState>();
            let mut previous = state.play.lock().unwrap();
            if previous.tug != play.tug || previous.tug_rounds != play.tug_rounds || previous.toy != play.toy || previous.last_catch != play.last_catch || previous.phase != play.phase || previous.catches != play.catches || previous.style != play.style {
                let _ = app.emit("pet:play", &play);
                if play.phase!="off" && (previous.phase=="off" || previous.toy!=play.toy) {let _=app.emit_to("playground","pet:game",if play.toy=="rope" {"tug"} else if play.toy=="frisbee" {"frisbee"} else {"fetch"});}
            }
            *previous = play;
            drop(previous);
            let encounter=engine.encounter_view();
            let mut previous=state.encounter.lock().unwrap();
            if *previous!=encounter { let _=app.emit("pet:encounter",&encounter); *previous=encounter; }
            drop(previous);

            // 穿透状态只在变化时下发，避免每帧都过一遍 IPC
            let clickable_changed=prev_clickable != Some(out.clickable);
            if clickable_changed {
                let _ = window.set_ignore_cursor_events(!out.clickable);
                prev_clickable = Some(out.clickable);
            }

            let frame = FramePayload {row:out.row,col:out.col,released_toy,petting:out.petting,dizzy:out.dizzy};
            if visible && prev_frame.as_ref()!=Some(&frame) {
                let _ = app.emit("pet:frame", &frame);
                prev_frame = Some(frame);
            }

            if prev_sleeping != Some(out.sleeping) || clickable_changed {
                let _ = app.emit("pet:state", StatePayload { sleeping: out.sleeping, clickable:out.clickable });
                prev_sleeping = Some(out.sleeping);
            }

            // 说话是离散事件，每 tick 至多一次，不需要去重
            if let Some(kind) = out.say {
                let reward=match kind {
                    engine::SayKind::PlayReturned=>Some(companion::Reward::Fetch),
                    engine::SayKind::Pat | engine::SayKind::BellyPat | engine::SayKind::Comfort=>Some(companion::Reward::Pat),_=>None
                };
                if let Some(reward)=reward {
                    if let Err(error)=update_memory(&app,|m|m.reward(reward,companion::now())) {
                        let _=app.emit("pet:memory-error",error);
                    }
                }
                let personal=match kind {
                    engine::SayKind::PlayReturned | engine::SayKind::Wake | engine::SayKind::Pat | engine::SayKind::BellyPat=>{
                        let stored=app.state::<AppState>();let stored=stored.memory.lock().unwrap();
                        if stored.error.is_none() && stored.data.affection>=20 && !stored.data.nickname.is_empty() {
                            let name=stored.data.address();
                            Some(match kind {
                                engine::SayKind::PlayReturned=>format!("{name}，叼回来啦！再玩一次？"),
                                engine::SayKind::Wake=>format!("{name}，我醒啦，继续陪你。"),
                                engine::SayKind::BellyPat=>format!("{name}，肚皮也交给你啦，好舒服。"),
                                _=>format!("{name}，最喜欢你摸摸头啦。")
                            })
                        } else { None }
                    },_=>None
                };
                let personal=if kind==engine::SayKind::PlayReturned && engine.play_view().streak>=3 {
                    Some(format!("连续接住 {} 次啦！再扔一次？",engine.play_view().streak))
                } else if kind==engine::SayKind::PlayReturned && engine.play_view().toy=="frisbee" && personal.is_none() {
                    Some("飞盘叼回来啦！再扔一次？".into())
                } else {personal};
                if let Some(text)=personal { let _=app.emit("pet:message",text); }
                else { let _=app.emit("pet:say",kind.as_str()); }
            }
        }
    });
}

/// 用户发起的操作失败时，隐藏窗口也能通过原生对话框看到原因。
pub(crate) fn report_error(app: &AppHandle, context: &str, error: impl std::fmt::Display) {
    let message = format!("{context}：{error}");
    eprintln!("{message}");
    let _ = app.emit("pet:error", &message);
    app.dialog().message(message).title("大熊：操作失败").kind(MessageDialogKind::Error).show(|_| {});
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(state) = app.try_state::<AppState>() {
                state.requested_visible.store(true, Ordering::Relaxed);
            }
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = window.show() { report_error(app, "显示已有宠物失败", e); }
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app,_shortcut,event| {
            if event.state==tauri_plugin_global_shortcut::ShortcutState::Pressed {
                if let Err(e)=open_play_window(app) {report_error(app,"快捷打开失败",e);}
            }
        }).build())
        .on_menu_event(tray::on_menu_event)
        .on_window_event(|window,event| {
            if window.label()=="playground" {if let tauri::WindowEvent::CloseRequested {api,..}=event {api.prevent_close();let _=window.hide();}}
        })
        .plugin(tauri_plugin_dialog::init())
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
            set_pomodoro,
            play_action,
            trick_status,
            snack_action,
            trick_action,
            reward_trick,
            play_status,
            set_visible,
            open_playground,
            playground_ready,
            open_pet_menu,close_playground,shortcut_status,set_shortcut,
            companion_status,
            set_nickname,
            feed_treat,
            restore_memory,
            memory_recovery_info,
            set_encounters,
            set_playful_fetch,
            set_quiet_companion,
            encounter_status
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").expect("缺少 main 窗口");

            // GTK 隐藏窗口尚未 realize 时，tao 的穿透实现会 unwrap 空 GdkWindow。
            // 只创建底层对象，不显示窗口，保留图片解码后才显示的启动流程。
            #[cfg(target_os = "linux")]
            {
                use gtk::prelude::WidgetExt;
                window.gtk_window()?.realize();
            }
            window.set_ignore_cursor_events(true)?;

            let saved = Config::load(app.handle()).unwrap_or_else(|e| {
                report_error(app.handle(), "读取配置失败，使用默认设置", e);
                Config::default()
            });

            let memory=match memory_path(app.handle()).and_then(|p|companion::Memory::load(&p).map_err(|e|e.to_string())) {
                Ok(data)=>MemoryState{data,error:None},
                Err(e)=>MemoryState{data:companion::Memory::default(),error:Some(format!("陪伴记忆读取失败：{e}"))},
            };

            // 扫描宠物包；一个都没有时用内置的兜底
            let pets = petpack::discover();

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
                requested_visible: AtomicBool::new(true), focusing:AtomicBool::new(false),
                play: Mutex::new(play::Play::default().view()),
                memory: Mutex::new(memory),
                activity: Mutex::new(activities::Activities::default().view()),rewarded_trick:AtomicU64::new(0),
                encounter: Mutex::new(encounters::Encounters::new(1).view()),
                shortcut:Mutex::new(ShortcutPrefs {requested:saved.shortcut_enabled,error:None}),
                pet_menu:Mutex::new(None),
            });

            if saved.shortcut_enabled {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Err(e)=app.global_shortcut().register(OPEN_SHORTCUT) {app.state::<AppState>().shortcut.lock().unwrap().error=Some(format!("快捷键未生效：{e}，仍可用右键菜单打开"));}
            }
            create_toy(app.handle())?;
            tray::build(app.handle(), &pets, &initial, saved.gravity)?;

            // 兜底：前端若因故没发出 ready，3 秒后也强制显示，不留一个隐形进程
            let fallback = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(3000));
                if let Some(w) = fallback.get_webview_window("main") {
                    if fallback.state::<AppState>().requested_visible.load(Ordering::Relaxed) {
                        if let Err(e) = w.show() { report_error(&fallback, "显示宠物失败", e); }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn state(pets: Vec<PetPack>) -> AppState {
        let (tx, _rx) = mpsc::channel();
        AppState {
            current: Mutex::new(pets[0].id.clone()), pets: Mutex::new(pets),
            look_enabled: Arc::new(AtomicBool::new(true)), gravity: Arc::new(AtomicBool::new(false)),
            sleep_frame: Arc::new(AtomicU16::new(pack_sleep(5, 2))), engine_tx: Mutex::new(tx),
            shortcut:Mutex::new(ShortcutPrefs::default()),
            pet_menu:Mutex::new(None),
            pet_revision: Arc::new(AtomicU64::new(0)), requested_visible: AtomicBool::new(true), focusing:AtomicBool::new(false),
                play: Mutex::new(play::Play::default().view()),
                activity: Mutex::new(activities::Activities::default().view()),rewarded_trick:AtomicU64::new(0),
                memory: Mutex::new(MemoryState{data:companion::Memory::default(),error:None}),
                encounter: Mutex::new(encounters::Encounters::new(1).view()),
        }
    }
    #[test]
    fn builtin_animation_atlas_matches_contract_and_preserves_original_art() {
        let original = image::load_from_memory(include_bytes!("../../public/spritesheet.webp")).unwrap().to_rgba8();
        let extended = image::load_from_memory(include_bytes!("../../public/spritesheet-extended.webp")).unwrap().to_rgba8();
        assert_eq!(extended.dimensions(), (1536, petpack::builtin().rows * 208));
        for (x, y, pixel) in original.enumerate_pixels() {
            let actual = extended.get_pixel(x, y);
            assert_eq!(actual[3], pixel[3]);
            if pixel[3] > 0 { assert_eq!(actual, pixel); }
        }
        // Roll starts and ends on the untouched canonical standing frame.
        for col in [0,7] {
            for y in 0..208 { for x in 0..192 {
                let a=original.get_pixel(x,y);let b=extended.get_pixel(col*192+x,26*208+y);
                assert_eq!(a[3],b[3]);if a[3]>0 { assert_eq!(a,b); }
            }}
        }
        for row in 11..petpack::builtin().rows {
            for col in 0..8 {
                let mut visible = 0;
                let mut min_y = 208;
                let mut max_y = 0;
                for y in 0..208 {
                    for x in 0..192 {
                        if extended.get_pixel(col * 192 + x, row * 208 + y)[3] > 40 {
                            visible += 1;
                            min_y = min_y.min(y);
                            max_y = max_y.max(y);
                            assert!(x >= 3 && x < 189 && y < 204, "frame {row}/{col} crosses padding");
                        }
                    }
                }
                assert!(visible > 5000, "frame {row}/{col} is empty");
                // Happy petting stays standing at the original height; never shrink into a puppy.
                if row == 11 || row == 20 { assert!(max_y - min_y + 1 >= 194, "standing frame {row}/{col} shrank"); }
            }
        }
    }

    #[test]
    fn missing_external_image_reports_error_and_does_not_masquerade_as_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = petpack::builtin();
        pack.id = "external".into();
        pack.rows = 9;
        pack.sheet = dir.path().join("missing.webp");
        let state = state(vec![pack]);
        assert!(payload_for(&state, "external").is_err());
        assert_eq!(*state.current.lock().unwrap(), "external");
    }
    #[test]
    fn builtin_payload_has_matching_dimensions_and_no_external_image() {
        let state = state(vec![petpack::builtin()]);
        let payload = payload_for(&state, "__builtin__").unwrap();
        assert_eq!(payload.rows, 27);
        assert!(payload.data_url.is_none());
        assert!(payload.speech.is_none());
    }
}
