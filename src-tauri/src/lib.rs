mod capture;
mod detect;
mod ocr;
mod perm;
mod settings;
mod translate;
mod util;
mod windows;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use settings::{Region, Settings};

/* ================================ 全局状态 ================================ */

pub struct AppState {
    pub config_dir: PathBuf,
    pub settings: Mutex<Settings>,
    /// 每个显示器最近一次的覆盖层初始化数据。
    /// 覆盖层刚创建时可能还没来得及注册事件监听，用它做兜底拉取。
    pub last_payload: Mutex<HashMap<u32, serde_json::Value>>,
}

impl AppState {
    fn new(config_dir: PathBuf) -> Self {
        let s = settings::load(&config_dir);
        Self {
            config_dir,
            settings: Mutex::new(s),
            last_payload: Mutex::new(HashMap::new()),
        }
    }
    fn get(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
    fn put(&self, s: &Settings) -> Result<(), String> {
        settings::save(&self.config_dir, s)
    }
}

/* ================================== 启动 ================================== */

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("shotly-config"));
            app.manage(AppState::new(config_dir));

            util::cleanup_old_temp();

            let handle = app.handle().clone();

            // ---- 托盘 ----
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
                .expect("内置图标损坏");
            let menu = MenuBuilder::new(app)
                .item(&MenuItemBuilder::with_id("capture", "区域截图").build(app)?)
                .item(&MenuItemBuilder::with_id("fixed", "固定区域截图").build(app)?)
                .item(&MenuItemBuilder::with_id("pin", "剪贴板固定").build(app)?)
                .separator()
                .item(&MenuItemBuilder::with_id("settings", "设置").build(app)?)
                .separator()
                .item(&MenuItemBuilder::with_id("quit", "退出 Shotly").build(app)?)
                .build()?;

            let tray_handle = handle.clone();
            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Shotly")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |_app, event| match event.id().as_ref() {
                    "capture" => { let _ = begin_capture(&tray_handle, false); }
                    "fixed" => { let _ = begin_capture(&tray_handle, true); }
                    "pin" => { let _ = pin_clipboard(tray_handle.clone()); }
                    "settings" => { let _ = open_settings(&tray_handle); }
                    "quit" => std::process::exit(0),
                    _ => {}
                })
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = begin_capture(&handle, false);
                    }
                })
                .build(app)?;

            // ---- 全局快捷键 ----
            let st = app.state::<AppState>().get();
            for (hotkey, kind) in [
                (st.capture_hotkey.clone(), "capture"),
                (st.fixed_hotkey.clone(), "fixed"),
                (st.pin_hotkey.clone(), "pin"),
            ] {
                let Ok(shortcut) = hotkey.parse::<Shortcut>() else {
                    eprintln!("[shotly] 快捷键无法解析: {hotkey}");
                    continue;
                };
                let h = app.handle().clone();
                let kind = kind.to_string();
                app.global_shortcut()
                    .on_shortcut(shortcut, move |_app, _sc, ev| {
                        if ev.state() != ShortcutState::Pressed {
                            return;
                        }
                        match kind.as_str() {
                            "capture" => { let _ = begin_capture(&h, false); }
                            "fixed" => { let _ = begin_capture(&h, true); }
                            _ => { let _ = pin_clipboard(h.clone()); }
                        }
                    })
                    .map_err(|e| e.to_string())?;
            }

            // 首次运行若未授权，直接把设置窗口打开，避免用户找不到入口
            let perm = perm::check();
            if !perm.granted {
                let _ = open_settings(app.handle());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 设置窗口与编辑器只是隐藏，保持后台常驻
                let label = window.label();
                if label == "main" || label == "editor" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings_cmd,
            check_permission_cmd,
            get_overlay_init,
            start_capture,
            capture_fixed_cmd,
            crop_and_save,
            close_overlays,
            open_editor_cmd,
            close_editor,
            open_pin_cmd,
            pin_clipboard_cmd,
            set_pin_click_through,
            copy_image_file,
            copy_text,
            save_image_as,
            save_png_from_data_url,
            read_png_base64,
            pick_color,
            ocr_cmd,
            translate_cmd,
            set_fixed_region,
            clear_fixed_region,
            open_settings_cmd,
            detect_element,
            check_ax_cmd,
        ])
        .build(tauri::generate_context!())
        .expect("Shotly 启动失败")
        .run(|_app, _event| {});
}

/* ================================ 核心流程 ================================ */

/// 抓取所有屏幕快照并打开选区覆盖层；fixed=true 时直接按已固定的区域出图。
fn begin_capture(app: &tauri::AppHandle, fixed: bool) -> Result<(), String> {
    let state = app.state::<AppState>();

    if fixed {
        let (id, region) = {
            let s = state.get();
            let found = s
                .fixed_regions
                .iter()
                .next()
                .map(|(k, v)| (k.clone(), v.clone()));
            match found {
                Some((k, v)) => (k.parse::<u32>().unwrap_or(0), v),
                None => return Err("还没有固定任何区域：先截图，然后在工具栏点「固定区域」".into()),
            }
        };
        let path = capture::capture_fixed_region(id, &region)?;
        return open_editor(app, &path, false);
    }

    let geometry = monitor_geometry(app);
    let snaps = capture::capture_all(&geometry)?;
    let fixed_map = state.get().fixed_regions.clone();

    for s in &snaps {
        let url = format!("index.html?win=overlay&m={}", s.id);
        let label = windows::overlay_label(s.id);
        let w = windows::ensure_overlay(app, s, &label, url)?;
        let payload = serde_json::json!({
            "id": s.id,
            "name": s.name,
            "path": s.path,
            "x": s.x,
            "y": s.y,
            "width": s.width,
            "height": s.height,
            "imgWidth": s.img_width,
            "imgHeight": s.img_height,
            "scale": s.scale,
            "fixed": fixed_map.get(&s.id.to_string()).cloned(),
            // 智能识别开关：悬停自动勾勒窗口/控件边框
            "smartDetect": state.get().smart_detect,
            "smartDetectLevel": state.get().smart_detect_level,
        });
        state
            .last_payload
            .lock()
            .unwrap()
            .insert(s.id, payload.clone());
        w.emit("overlay://init", payload).map_err(|e| e.to_string())?;
        w.show().map_err(|e| e.to_string())?;
        w.unminimize().ok();
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn monitor_geometry(app: &tauri::AppHandle) -> Vec<(i32, i32, u32, u32, f64)> {
    match app.available_monitors() {
        Ok(ms) => ms
            .iter()
            .map(|m| {
                let p = m.position();
                let s = m.size();
                (p.x, p.y, s.width, s.height, m.scale_factor())
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn open_settings(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.show().map_err(|e| e.to_string())?;
        w.unminimize().ok();
        w.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::App("index.html?win=settings".into()),
    )
    .title("Shotly 设置")
    .inner_size(720.0, 780.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn open_editor(app: &tauri::AppHandle, path: &str, do_ocr: bool) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("editor") {
        w.emit(
            "editor://load",
            serde_json::json!({ "src": path, "ocr": do_ocr }),
        )
        .map_err(|e| e.to_string())?;
        w.show().map_err(|e| e.to_string())?;
        w.unminimize().ok();
        w.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let url = format!(
        "index.html?win=editor&src={}&ocr={}",
        util::urlencode(path),
        if do_ocr { 1 } else { 0 }
    );
    WebviewWindowBuilder::new(app, "editor", WebviewUrl::App(url.into()))
        .title("Shotly")
        .inner_size(1180.0, 780.0)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn open_pin(app: &tauri::AppHandle, path: &str) -> Result<(), String> {
    let (w, h) = util::image_size(path)?;
    let sf = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor() as f32)
        .unwrap_or(2.0);

    let (mx, my, mw, mh) = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let p = m.position();
            let s = m.size();
            (p.x, p.y, s.width, s.height)
        })
        .unwrap_or((0, 0, 1920, 1080));

    // 全部按物理像素计算，主屏居中
    let x = mx + ((mw as i32 - w as i32) / 2).max(0);
    let y = my + ((mh as i32 - h as i32) / 2).max(0);

    let label = format!("pin-{}", util::stamp());
    let url = format!(
        "index.html?win=pin&src={}&w={}&h={}",
        util::urlencode(path),
        w,
        h
    );

    let builder = windows::with_transparent(
        WebviewWindowBuilder::new(app, label.clone(), WebviewUrl::App(url.into()))
            .title("Shotly 固定")
            .inner_size(w as f64 / sf as f64, h as f64 / sf as f64)
            .position(x as f64 / sf as f64, y as f64 / sf as f64)
            .decorations(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .resizable(true)
            .visible(true),
    );
    let win = builder.build().map_err(|e| e.to_string())?;
    // 建好后再用物理像素精确对位，1:1 显示原图
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(w, h))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/* ================================= 命令 ================================= */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageOut {
    path: String,
    width: u32,
    height: u32,
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.get()
}

#[tauri::command]
fn save_settings_cmd(settings: Settings, state: tauri::State<AppState>) -> Result<(), String> {
    state.put(&settings)
}

#[tauri::command]
fn check_permission_cmd() -> perm::Permission {
    perm::check()
}

/// 覆盖层刚创建时可能错过事件，用它补拉一次初始化数据。
#[tauri::command]
fn get_overlay_init(
    state: tauri::State<AppState>,
    monitor_id: u32,
) -> Option<serde_json::Value> {
    state.last_payload.lock().unwrap().get(&monitor_id).cloned()
}

#[tauri::command]
fn start_capture(app: tauri::AppHandle) -> Result<(), String> {
    begin_capture(&app, false)
}

#[tauri::command]
fn capture_fixed_cmd(app: tauri::AppHandle) -> Result<(), String> {
    begin_capture(&app, true)
}

#[tauri::command]
fn crop_and_save(path: String, x: i32, y: i32, w: u32, h: u32) -> Result<ImageOut, String> {
    let img = image::open(&path).map_err(|e| format!("打开快照失败: {e}"))?;
    let rgba = img.to_rgba8();
    let sx = x.max(0) as u32;
    let sy = y.max(0) as u32;
    let cw = w.min(rgba.width().saturating_sub(sx)).max(1);
    let ch = h.min(rgba.height().saturating_sub(sy)).max(1);
    let out_img = image::imageops::crop_imm(&rgba, sx, sy, cw, ch).to_image();
    let out = util::new_temp_file("png");
    out_img.save(&out).map_err(|e| format!("保存截图失败: {e}"))?;
    Ok(ImageOut {
        path: util::path_to_string(&out),
        width: cw,
        height: ch,
    })
}

#[tauri::command]
fn close_overlays(app: tauri::AppHandle) -> Result<(), String> {
    for (label, w) in app.webview_windows() {
        if label.starts_with(windows::OVERLAY_PREFIX) {
            let _ = w.hide();
            let _ = w.set_ignore_cursor_events(false);
        }
    }
    Ok(())
}

#[tauri::command]
fn open_editor_cmd(app: tauri::AppHandle, path: String, ocr: Option<bool>) -> Result<(), String> {
    open_editor(&app, &path, ocr.unwrap_or(false))
}

#[tauri::command]
fn close_editor(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("editor") {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_pin_cmd(app: tauri::AppHandle, path: String) -> Result<(), String> {
    open_pin(&app, &path)
}

#[tauri::command]
fn pin_clipboard_cmd(app: tauri::AppHandle) -> Result<(), String> {
    pin_clipboard(app)
}

fn pin_clipboard(app: tauri::AppHandle) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("剪贴板不可用: {e}"))?;
    let img = cb
        .get_image()
        .map_err(|_| "剪贴板里没有图片".to_string())?;
    let rgba =
        image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
            .ok_or_else(|| "剪贴板图片格式不支持".to_string())?;
    let path = util::new_temp_file("png");
    rgba.save(&path).map_err(|e| format!("保存失败: {e}"))?;
    open_pin(&app, &util::path_to_string(&path))
}

#[tauri::command]
fn set_pin_click_through(app: tauri::AppHandle, label: String, through: bool) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(&label) {
        w.set_ignore_cursor_events(through).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn copy_image_file(path: String) -> Result<(), String> {
    let img = image::open(&path).map_err(|e| format!("打开图片失败: {e}"))?;
    let rgba = img.to_rgba8();
    let data = arboard::ImageData {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        bytes: std::borrow::Cow::from(rgba.into_raw()),
    };
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("剪贴板不可用: {e}"))?;
    cb.set_image(data).map_err(|e| format!("写入剪贴板失败: {e}"))
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("剪贴板不可用: {e}"))?;
    cb.set_text(text).map_err(|e| format!("写入剪贴板失败: {e}"))
}

#[tauri::command]
fn save_image_as(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    path: String,
) -> Result<Option<String>, String> {
    let dir = state.get().save_dir;
    let dest: Option<PathBuf> = if dir.trim().is_empty() {
        app.dialog()
            .file()
            .add_filter("PNG 图片", &["png"])
            .set_file_name(&format!("shotly-{}.png", util::stamp()))
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    } else {
        let d = PathBuf::from(dir.trim());
        let _ = std::fs::create_dir_all(&d);
        Some(d.join(format!("shotly-{}.png", util::stamp())))
    };
    let Some(dest) = dest else { return Ok(None) };
    std::fs::copy(&path, &dest).map_err(|e| format!("保存失败: {e}"))?;
    Ok(Some(dest.to_string_lossy().to_string()))
}

#[tauri::command]
fn save_png_from_data_url(data_url: String) -> Result<ImageOut, String> {
    let p = util::write_data_url(&data_url)?;
    let (w, h) = util::image_size(&util::path_to_string(&p))?;
    Ok(ImageOut { path: util::path_to_string(&p), width: w, height: h })
}

#[tauri::command]
fn read_png_base64(path: String) -> Result<String, String> {
    util::read_base64(&path)
}

#[tauri::command]
fn pick_color(path: String, x: u32, y: u32) -> Result<String, String> {
    let img = image::open(&path).map_err(|e| format!("打开图片失败: {e}"))?;
    let rgba = img.to_rgba8();
    if x >= rgba.width() || y >= rgba.height() {
        return Ok("#000000".into());
    }
    let p = rgba.get_pixel(x, y);
    Ok(format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]))
}

#[tauri::command]
fn ocr_cmd(app: tauri::AppHandle, path: String) -> Result<ocr::OcrResult, String> {
    let langs = app.state::<AppState>().get().ocr_lang;
    // 未配置语言时回退到中文 + 英文，避免 Vision 因空语言列表而识别为空
    let langs = if langs.trim().is_empty() {
        "zh-Hans,en-US".to_string()
    } else {
        langs
    };
    ocr::recognize(&path, &langs)
}

#[tauri::command]
fn translate_cmd(app: tauri::AppHandle, text: String, target: String) -> Result<String, String> {
    let s = app.state::<AppState>().get();
    translate::translate(&text, &target, &s)
}

#[tauri::command]
fn set_fixed_region(
    app: tauri::AppHandle,
    monitor_id: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut s = state.get();
    s.fixed_regions
        .insert(monitor_id.to_string(), Region { x, y, w, h });
    state.put(&s)
}

#[tauri::command]
fn clear_fixed_region(app: tauri::AppHandle, monitor_id: u32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut s = state.get();
    s.fixed_regions.remove(&monitor_id.to_string());
    state.put(&s)
}

#[tauri::command]
fn open_settings_cmd(app: tauri::AppHandle) -> Result<(), String> {
    open_settings(&app)
}

/* ============================= 窗口/控件智能识别 ============================= */

#[tauri::command]
fn detect_element(x: f64, y: f64, level: String) -> Option<detect::DetectedRect> {
    detect::detect_element(x, y, &level)
}

#[tauri::command]
fn check_ax_cmd() -> bool {
    detect::ax_trusted()
}
