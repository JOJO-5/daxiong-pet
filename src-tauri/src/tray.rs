//! 系统托盘。窗口不在任务栏显示，托盘是唯一的常驻入口。
//! 宠物列表与开关状态都是动态的，所以菜单每次按当前状态重建。

use crate::petpack::PetPack;
use std::sync::atomic::Ordering;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuBuilder, MenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};
use tauri_plugin_autostart::ManagerExt;

pub const TRAY_ID: &str = "daxiong-tray";

fn toggle_window(app: &AppHandle) {
    if app.get_webview_window("main").is_some() {
        let visible = app.state::<crate::AppState>().requested_visible.load(Ordering::Relaxed);
        if let Err(e)=crate::apply_visibility(app,!visible) { crate::report_error(app,"显示 / 隐藏宠物失败",e); }
    }
}

fn open_pet_dir() -> std::io::Result<()> {
    let roots = crate::petpack::search_roots();
    let dir = roots.iter().find(|p| p.is_dir()).or_else(|| roots.first())
        .ok_or_else(|| std::io::Error::other("找不到宠物目录"))?;
    std::fs::create_dir_all(dir)?;
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    std::process::Command::new(opener).arg(dir).spawn()?;
    Ok(())
}

/// 当前的重力 / 自启状态，供菜单勾选用
fn current_flags(app: &AppHandle) -> (bool, bool) {
    let gravity = app
        .state::<crate::AppState>()
        .gravity
        .load(Ordering::Relaxed);
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    (gravity, autostart)
}

/// 按当前宠物列表与开关状态重建菜单
fn build_menu(app: &AppHandle, pets: &[PetPack], current: &str) -> tauri::Result<Menu<Wry>> {
    let (gravity, autostart_on) = current_flags(app);

    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏", true, None::<&str>)?;
    let rescan = MenuItem::with_id(app, "rescan", "重新扫描宠物", true, None::<&str>)?;
    let open_dir = MenuItem::with_id(app, "open_dir", "打开宠物文件夹", true, None::<&str>)?;
    let pomodoro = MenuItem::with_id(
        app,
        "pomodoro",
        "专注 25 分钟（再点即取消）",
        true,
        None::<&str>,
    )?;
    let play = MenuItem::with_id(app, "play", "和大熊一起玩…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let gravity_item = CheckMenuItem::with_id(
        app,
        "gravity",
        "重力（松开手会往下掉）",
        true,
        gravity,
        None::<&str>,
    )?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机自动启动",
        true,
        autostart_on,
        None::<&str>,
    )?;

    // 宠物列表：当前项用 ● 标出；行数一并显示，方便一眼看出有没有注视
    let mut pet_menu = SubmenuBuilder::new(app, "切换宠物");
    for pack in pets {
        let mark = if pack.id == current { "●" } else { "　" };
        let label = format!("{} {}（{} 行）", mark, pack.name, pack.rows);
        let item = MenuItem::with_id(app, format!("pet:{}", pack.id), label, true, None::<&str>)?;
        pet_menu = pet_menu.item(&item);
    }

    MenuBuilder::new(app)
        .item(&toggle)
        .separator()
        .item(&pet_menu.build()?)
        .item(&rescan)
        .item(&open_dir)
        .separator()
        .item(&play)
        .item(&pomodoro)
        .item(&gravity_item)
        .separator()
        .item(&autostart)
        .separator()
        .item(&quit)
        .build()
}

pub fn build(app: &AppHandle, pets: &[PetPack], current: &str, _gravity: bool) -> tauri::Result<()> {
    let menu = build_menu(app, pets, current)?;
    let icon = app
        .default_window_icon()
        .expect("缺少默认窗口图标")
        .clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("桌面宠物")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// 列表或开关变化后刷新菜单
pub fn refresh(app: &AppHandle, pets: &[PetPack], current: &str, _gravity: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let result = build_menu(app, pets, current).and_then(|menu| tray.set_menu(Some(menu)));
    if let Err(e) = result { crate::report_error(app, "更新托盘菜单失败", e); }
}

pub(crate) fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();

    // 宠物切换项统一以 pet: 前缀命名
    if let Some(pet_id) = id.strip_prefix("pet:") {
        if let Err(err) = crate::switch_to_pet(app, pet_id) {
            crate::report_error(app, "切换宠物失败", err);
        }
        return;
    }

    match id {
        "pet_feed"=>{if let Err(e)=crate::feed_treat(app.clone()) {crate::report_error(app,"投喂失败",e);}},
        "pet_ball" | "pet_frisbee" | "pet_tug" | "pet_stop"=>{let action=match id {"pet_ball"=>"throw","pet_frisbee"=>"throw_frisbee","pet_tug"=>"start_tug",_=>"cancel"};if let Err(e)=crate::play_action(action.into(),app.clone()) {crate::report_error(app,"互动失败",e);}},
        "pet_come"=>{if let Err(e)=crate::trick_action("come".into(),app.clone()) {crate::report_error(app,"指令失败",e);}},
        "pet_preferences"=>{if let Err(e)=crate::open_preferences(app) {crate::report_error(app,"打开偏好失败",e);}},
        "toggle" => toggle_window(app),
        "play" => { if let Err(e) = crate::open_play_window(app) { crate::report_error(app,"打开互动面板失败",e); } },
        "rescan" => {
            if let Err(err) = crate::rescan_and_refresh(app) {
                crate::report_error(app, "重新扫描失败", err);
            }
        }
        "open_dir" => { if let Err(e) = open_pet_dir() { crate::report_error(app, "打开宠物目录失败", e); } },

        "pomodoro" => {
            let state = app.state::<crate::AppState>();
            let tx = state.engine_tx.lock().unwrap();
            if let Err(e) = tx.send(crate::engine::Command::TogglePomodoro) { crate::report_error(app, "番茄钟操作失败", e); }
        }

        "gravity" => {
            let state = app.state::<crate::AppState>();
            let now = state.gravity.load(Ordering::Relaxed);
            if let Err(e) = crate::apply_gravity(app, !now) { crate::report_error(app, "保存重力设置失败", e); }
            // 重建菜单，勾选状态跟着系统实际值走
            let pets = state.pets.lock().unwrap().clone();
            let current = state.current.lock().unwrap().clone();
            refresh(app, &pets, &current, !now);
        }

        "autostart" => {
            let launcher = app.autolaunch();
            let enabled = match launcher.is_enabled() {
                Ok(v) => v,
                Err(e) => { crate::report_error(app, "读取自启状态失败", e); return; }
            };
            let result = if enabled {
                launcher.disable()
            } else {
                launcher.enable()
            };
            if let Err(e) = result { crate::report_error(app, "设置开机自启失败", e); }
            let state = app.state::<crate::AppState>();
            let pets = state.pets.lock().unwrap().clone();
            let current = state.current.lock().unwrap().clone();
            let gravity = state.gravity.load(Ordering::Relaxed);
            refresh(app, &pets, &current, gravity);
        }

        "quit" => app.exit(0),
        _ => {}
    }
}
