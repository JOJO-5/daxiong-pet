//! Windows 平台层：全局光标、鼠标按键、本地时间和显示器工作区。
//!
//! 之所以不用前端事件：窗口在非命中区域是「点击穿透」的，鼠标事件根本到不了
//! WebView，所以按键状态必须从系统层面读取。

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{POINT, SYSTEMTIME};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

#[cfg(target_os = "windows")]
const VK_LBUTTON: i32 = 0x01;

/// 全局光标位置（虚拟屏幕物理像素坐标）。
#[cfg(target_os = "windows")]
pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: GetCursorPos 只写入我们提供的栈上 POINT。
    unsafe {
        GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

/// 鼠标左键当前是否按下。
#[cfg(target_os = "windows")]
pub fn primary_button_down() -> bool {
    // SAFETY: GetAsyncKeyState 无参数副作用，纯查询。
    unsafe { (GetAsyncKeyState(VK_LBUTTON) as u16 & 0x8000) != 0 }
}

/// 本地时间的「小时」（0-23），用于整点报时。
#[cfg(target_os = "windows")]
pub fn local_hour() -> u32 {
    let mut st = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    // SAFETY: GetLocalTime 只写入我们提供的栈上 SYSTEMTIME。
    unsafe {
        GetLocalTime(&mut st);
    }
    st.wHour as u32
}


/// 前端和窗口坐标统一由 Tauri 转为物理像素。
pub fn pointer_state(window: &tauri::WebviewWindow) -> ((i32, i32), bool) {
    #[cfg(target_os = "windows")]
    {
        let _ = window;
        (cursor_pos(), primary_button_down())
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use device_query::DeviceQuery;
        thread_local! {
            // 无权限 / 无 X11 时不让后台引擎 panic；每秒重试以支持稍后授权。
            static DEVICE: std::cell::RefCell<(Option<device_query::DeviceState>, std::time::Instant)> =
                std::cell::RefCell::new((device_query::DeviceState::checked_new(), std::time::Instant::now()));
        }
        let pressed = DEVICE.with(|cell| {
            let mut state = cell.borrow_mut();
            if state.0.is_none() && state.1.elapsed().as_secs() >= 1 {
                state.0 = device_query::DeviceState::checked_new();
                state.1 = std::time::Instant::now();
            }
            state.0.as_ref().map(|device| device.get_mouse().button_pressed.get(1).copied().unwrap_or(false)).unwrap_or(false)
        });
        let cursor = window.cursor_position().map(|p| (p.x.round() as i32, p.y.round() as i32))
            .unwrap_or((i32::MIN / 4, i32::MIN / 4));
        (cursor, pressed)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn local_hour() -> u32 {
    use chrono::Timelike;
    chrono::Local::now().hour()
}
