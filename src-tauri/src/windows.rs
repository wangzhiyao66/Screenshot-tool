use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::capture::Snapshot;

pub const OVERLAY_PREFIX: &str = "overlay-";

pub fn overlay_label(id: u32) -> String {
    format!("{OVERLAY_PREFIX}{id}")
}

/// macOS 上透明窗口是默认行为，`transparent()` 只在非 macOS（或开启
/// macos-private-api）时存在，这里做编译期分叉。
pub fn with_transparent<'a, R: tauri::Runtime, M: Manager<R>>(
    b: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    {
        b
    }
    #[cfg(not(target_os = "macos"))]
    {
        b.transparent(true)
    }
}

/// 覆盖层窗口常驻复用：只在第一次创建，之后仅更新几何并 show/hide，
/// 避免每次截图都新建 WebView（那会带来 200ms 以上的延迟）。
pub fn ensure_overlay(
    app: &AppHandle,
    s: &Snapshot,
    label: &str,
    url: String,
) -> Result<WebviewWindow, String> {
    let w = s.width.max(s.img_width);
    let h = s.height.max(s.img_height);

    if let Some(win) = app.get_webview_window(label) {
        place(&win, s.x, s.y, w, h)?;
        return Ok(win);
    }

    let sf = if s.scale > 0.0 { s.scale as f64 } else { 1.0 };
    let builder = with_transparent(
        WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
            .title("Shotly")
            .inner_size(w as f64 / sf, h as f64 / sf)
            .position(s.x as f64 / sf, s.y as f64 / sf)
            .decorations(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .resizable(false)
            .visible(false),
    );
    let win = builder.build().map_err(|e| e.to_string())?;
    // 构建器只接受逻辑像素，建好后再用物理像素精确对位
    place(&win, s.x, s.y, w, h)?;
    Ok(win)
}

fn place(win: &WebviewWindow, x: i32, y: i32, w: u32, h: u32) -> Result<(), String> {
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(w, h))
        .map_err(|e| e.to_string())?;
    Ok(())
}
