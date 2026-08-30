use serde::Serialize;
use xcap::Monitor;

use crate::settings::Region;
use crate::util;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: u32,
    pub name: String,
    pub path: String,
    /// 显示器原点（物理像素，可能为负）
    pub x: i32,
    pub y: i32,
    /// 显示器物理尺寸
    pub width: u32,
    pub height: u32,
    /// 快照图片的真实像素尺寸
    pub img_width: u32,
    pub img_height: u32,
    pub scale: f32,
}

/// 抓取所有显示器的静态快照，保存到临时目录并返回元信息。
///
/// 窗口几何优先采用 Tauri 提供的物理坐标（跨平台语义明确），
/// 取不到时退回 xcap 自己的坐标。
pub fn capture_all(monitors: &[(i32, i32, u32, u32, f64)]) -> Result<Vec<Snapshot>, String> {
    let list = Monitor::all().map_err(|e| format!("枚举显示器失败: {e}"))?;
    let mut out = Vec::with_capacity(list.len());

    for (i, m) in list.iter().enumerate() {
        let img = m.capture_image().map_err(|e| format!("截屏失败: {e}"))?;
        let path = util::new_temp_file("png");
        img.save(&path).map_err(|e| format!("保存快照失败: {e}"))?;

        let (iw, ih) = (img.width(), img.height());
        let (x, y, w, h, scale) = monitors
            .get(i)
            .copied()
            .unwrap_or_else(|| {
                (
                    m.x().unwrap_or(0),
                    m.y().unwrap_or(0),
                    iw,
                    ih,
                    m.scale_factor().unwrap_or(1.0) as f64,
                )
            });

        out.push(Snapshot {
            id: m.id().unwrap_or(i as u32),
            name: m.friendly_name().unwrap_or_else(|_| format!("显示器 {}", i + 1)),
            path: util::path_to_string(&path),
            x,
            y,
            width: w,
            height: h,
            img_width: iw,
            img_height: ih,
            scale: scale as f32,
        });
    }

    if out.is_empty() {
        return Err("没有可用的显示器".into());
    }
    Ok(out)
}

/// 直接按固定区域截取某个显示器，返回裁剪后的 PNG 路径。
pub fn capture_fixed_region(monitor_id: u32, r: &Region) -> Result<String, String> {
    let list = Monitor::all().map_err(|e| format!("枚举显示器失败: {e}"))?;
    let m = list
        .iter()
        .find(|m| m.id().unwrap_or(u32::MAX) == monitor_id)
        .or_else(|| list.first())
        .ok_or_else(|| "没有可用的显示器".to_string())?;

    // 先整屏抓到原生分辨率，再在内存里裁剪，避免 DPI 二次缩放导致的偏移
    let img = m.capture_image().map_err(|e| format!("截屏失败: {e}"))?;
    let sx = r.x.max(0) as u32;
    let sy = r.y.max(0) as u32;
    let w = (r.w as u32).min(img.width().saturating_sub(sx));
    let h = (r.h as u32).min(img.height().saturating_sub(sy));
    if w == 0 || h == 0 {
        return Err("固定区域已超出当前屏幕范围".into());
    }

    let cropped = image::imageops::crop_imm(&img, sx, sy, w, h).to_image();
    let path = util::new_temp_file("png");
    cropped
        .save(&path)
        .map_err(|e| format!("保存截图失败: {e}"))?;
    Ok(util::path_to_string(&path))
}
