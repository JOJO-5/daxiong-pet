//! 平台层：只需要三个 Windows API —— 取全局光标位置、取鼠标左键状态、取本地时间。
//!
//! 之所以不用前端事件：窗口在非命中区域是「点击穿透」的，鼠标事件根本到不了
//! WebView，所以按键状态必须从系统层面读取。

use windows_sys::Win32::Foundation::{POINT, SYSTEMTIME};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

const VK_LBUTTON: i32 = 0x01;

/// 全局光标位置（虚拟屏幕物理像素坐标）。
pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: GetCursorPos 只写入我们提供的栈上 POINT。
    unsafe {
        GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

/// 鼠标左键当前是否按下。
pub fn primary_button_down() -> bool {
    // SAFETY: GetAsyncKeyState 无参数副作用，纯查询。
    unsafe { (GetAsyncKeyState(VK_LBUTTON) as u16 & 0x8000) != 0 }
}

/// 本地时间的「小时」（0-23），用于整点报时。
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
