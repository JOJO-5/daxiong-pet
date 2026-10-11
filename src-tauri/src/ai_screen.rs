//! Authorized, single-shot capture. Screenshots and OCR live only in memory.
use crate::ai_store::{Result, Settings};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde::Serialize;
use std::io::Cursor;
#[derive(Clone, Serialize)]
pub struct Display {
    pub id: u32,
    pub name: String,
    pub primary: bool,
}
pub struct Capture {
    pub png: Vec<u8>,
    pub identity: String,
    pub label: String,
    pub fingerprint: Vec<u8>,
}
pub fn displays() -> Result<Vec<Display>> {
    xcap::Monitor::all()
        .map_err(|_| "无法读取显示器信息。")?
        .into_iter()
        .map(|m| {
            Ok(Display {
                id: m.id().map_err(|_| "显示器信息不可用。")?,
                name: m.name().unwrap_or_else(|_| "显示器".into()),
                primary: m.is_primary().unwrap_or(false),
            })
        })
        .collect()
}
pub fn permission(request: bool) -> Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::StationsAndDesktops::{
            CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_READOBJECTS,
            UOI_NAME,
        };
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return Err("当前桌面处于锁屏、权限提示或不可捕获状态。".into());
        }
        let mut buffer = [0_u16; 256];
        let mut needed = 0_u32;
        let okay = GetUserObjectInformationW(
            desktop,
            UOI_NAME,
            buffer.as_mut_ptr() as *mut _,
            std::mem::size_of_val(&buffer) as u32,
            &mut needed,
        );
        CloseDesktop(desktop);
        let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        if okay == 0 || !String::from_utf16_lossy(&buffer[..end]).eq_ignore_ascii_case("default") {
            return Err("安全桌面或锁屏期间不会观察。".into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
            fn CGRequestScreenCaptureAccess() -> bool;
        }
        unsafe {
            if !CGPreflightScreenCaptureAccess() {
                if !request || !CGRequestScreenCaptureAccess() {
                    return Err("请在系统设置的屏幕录制权限中允许大熊，再重试。".into());
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland")
            || std::env::var_os("WAYLAND_DISPLAY").is_some()
        {
            return Err(
                "当前Wayland会话尚不支持不落盘的单次捕获；请使用X11会话，聊天仍可使用。".into(),
            );
        }
    }
    let _ = request;
    Ok(())
}
fn foreground(settings: &Settings) -> Result<xcap::Window> {
    let windows = xcap::Window::all().map_err(|_| "前台窗口不可读取，可能是锁屏或权限不足。")?;
    let window = windows
        .into_iter()
        .find(|w| w.is_focused().unwrap_or(false))
        .ok_or("没有可捕获的前台窗口。")?;
    if window.pid().unwrap_or(std::process::id()) == std::process::id()
        || window.is_minimized().unwrap_or(true)
    {
        return Err("请切换到要观察的应用窗口；大熊不会观察自己的窗口。".into());
    }
    let app = window.app_name().unwrap_or_default().to_lowercase();
    if [
        "logonui",
        "lockapp",
        "i3lock",
        "xscreensaver",
        "gnome-screensaver",
        "kscreenlocker",
    ]
    .iter()
    .any(|s| app.contains(s))
        || settings
            .excluded_apps
            .iter()
            .any(|s| !s.is_empty() && app.contains(&s.to_lowercase()))
    {
        return Err("当前应用已排除或处于锁屏状态。".into());
    }
    Ok(window)
}
pub fn identity(settings: &Settings) -> Result<String> {
    permission(false)?;
    let w = foreground(settings)?;
    Ok(format!(
        "{}:{}:{}:{}",
        settings.screen_scope,
        settings
            .monitor_id
            .map(|x| x.to_string())
            .unwrap_or_default(),
        w.id().map_err(|_| "窗口标识不可用。")?,
        w.title().unwrap_or_default()
    ))
}
pub fn capture(settings: &Settings, request: bool) -> Result<Capture> {
    permission(request)?;
    let window = foreground(settings)?;
    let identity = identity(settings)?;
    let origin: (i64, i64);
    let (mut image, label) = if settings.screen_scope == "window" {
        origin = (
            window.x().map_err(|_| "窗口位置不可用。")? as i64,
            window.y().map_err(|_| "窗口位置不可用。")? as i64,
        );
        (
            window
                .capture_image()
                .map_err(|_| "窗口捕获失败，请检查屏幕权限或切换窗口。")?,
            format!(
                "前台窗口：{}",
                window.title().unwrap_or_else(|_| "未命名窗口".into())
            ),
        )
    } else {
        let mut monitors = xcap::Monitor::all().map_err(|_| "显示器捕获不可用。")?;
        if settings.screen_scope == "monitor" {
            let index = monitors
                .iter()
                .position(|m| {
                    settings
                        .monitor_id
                        .map(|id| m.id().ok() == Some(id))
                        .unwrap_or_else(|| m.is_primary().unwrap_or(false))
                })
                .ok_or("选择的显示器已断开。")?;
            let monitor = monitors.remove(index);
            origin = (
                monitor.x().map_err(|_| "显示器位置不可用。")? as i64,
                monitor.y().map_err(|_| "显示器位置不可用。")? as i64,
            );
            (
                monitor.capture_image().map_err(|_| "显示器捕获失败。")?,
                format!("整个显示器：{}", monitor.name().unwrap_or_default()),
            )
        } else {
            let mut parts = Vec::new();
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
            for m in monitors {
                let x = m.x().map_err(|_| "显示器位置不可用。")? as i64;
                let y = m.y().map_err(|_| "显示器位置不可用。")? as i64;
                let image = m.capture_image().map_err(|_| "部分显示器捕获失败。")?;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + image.width() as i64);
                max_y = max_y.max(y + image.height() as i64);
                parts.push((x, y, image));
            }
            let (w, h) = (max_x.saturating_sub(min_x), max_y.saturating_sub(min_y));
            if w <= 0 || h <= 0 || w > 16384 || h > 16384 || w * h > 32 * 1024 * 1024 {
                return Err("全部显示器范围过大，请选择单个显示器。".into());
            }
            origin = (min_x, min_y);
            let mut canvas = RgbaImage::new(w as u32, h as u32);
            for (x, y, image) in parts {
                image::imageops::overlay(&mut canvas, &image, x - min_x, y - min_y);
            }
            (canvas, "整个桌面：全部显示器".into())
        }
    };
    if image.width() == 0
        || image.height() == 0
        || image.width() as u64 * image.height() as u64 > 32 * 1024 * 1024
    {
        return Err("画面尺寸超出允许范围。".into());
    }
    {
        for own in xcap::Window::all().map_err(|_| "无法核对需要排除的窗口。")? {
            let app = own.app_name().unwrap_or_default().to_lowercase();
            let excluded = own.pid().ok() == Some(std::process::id())
                || settings
                    .excluded_apps
                    .iter()
                    .any(|s| !s.is_empty() && app.contains(&s.to_lowercase()));
            if !excluded || own.is_minimized().unwrap_or(true) {
                continue;
            }
            // A foreground capture must not mask apps behind the target.
            if settings.screen_scope == "window"
                && own.z().map_err(|_| "窗口层级不可用。")?
                    <= window.z().map_err(|_| "窗口层级不可用。")?
            {
                continue;
            }
            let x = own.x().map_err(|_| "无法遮蔽排除窗口。")? as i64 - origin.0;
            let y = own.y().map_err(|_| "无法遮蔽排除窗口。")? as i64 - origin.1;
            let width = own.width().map_err(|_| "无法遮蔽排除窗口。")? as i64;
            let height = own.height().map_err(|_| "无法遮蔽排除窗口。")? as i64;
            for row in y.max(0)..(y + height).min(image.height() as i64) {
                for col in x.max(0)..(x + width).min(image.width() as i64) {
                    image.put_pixel(col as u32, row as u32, image::Rgba([128, 128, 128, 255]));
                }
            }
        }
    }
    // Scale in memory; never write captured pixels to temporary files.
    let image = DynamicImage::ImageRgba8(image).thumbnail(1920, 1920);
    let fingerprint = image
        .resize_exact(32, 18, image::imageops::FilterType::Triangle)
        .to_luma8()
        .into_raw();
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|_| "画面编码失败。")?;
    if self::identity(settings).ok().as_deref() != Some(identity.as_str()) {
        return Err("观察期间窗口改变，请重新观察。".into());
    }
    Ok(Capture {
        png: png.into_inner(),
        identity,
        label,
        fingerprint,
    })
}
pub fn changed(old: &[u8], new: &[u8]) -> bool {
    old.len() != new.len()
        || old
            .iter()
            .zip(new)
            .filter(|(a, b)| a.abs_diff(**b) > 10)
            .count()
            > old.len() / 32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minor_cursor_noise_is_not_a_new_screen() {
        let a = vec![100; 576];
        let mut b = a.clone();
        b[1] = 150;
        assert!(!changed(&a, &b));
        for v in &mut b[..30] {
            *v = 140;
        }
        assert!(changed(&a, &b));
        assert!(changed(&a, &[]));
    }
}
