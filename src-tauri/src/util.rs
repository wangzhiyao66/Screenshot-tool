use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 临时文件目录（macOS: $TMPDIR，Windows: %TEMP%），按进程生命周期使用。
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("shotly")
}

pub fn ensure_temp_dir() -> PathBuf {
    let d = temp_dir();
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn new_temp_file(ext: &str) -> PathBuf {
    let d = ensure_temp_dir();
    let pid = std::process::id();
    let r: u32 = rand::random();
    d.join(format!("shotly-{}-{}-{}.{}", pid, stamp(), r, ext))
}

pub fn path_to_string(p: &std::path::Path) -> String {
    p.to_string_lossy().to_string()
}

pub fn read_base64(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取文件失败: {e}"))?;
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        bytes,
    ))
}

/// 把 `data:image/png;base64,xxxx` 写入临时文件，返回路径。
pub fn write_data_url(data_url: &str) -> Result<PathBuf, String> {
    use base64::Engine;
    let b64 = data_url
        .split_once(',')
        .map(|(_, b)| b)
        .unwrap_or(data_url);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64 解码失败: {e}"))?;
    let out = new_temp_file("png");
    std::fs::write(&out, bytes).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(out)
}

/// 用于拼 URL 查询参数（比 percent-encoding 宽松，但足够覆盖路径里的空格与中文）
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn image_size(path: &str) -> Result<(u32, u32), String> {
    let (w, h) = image::image_dimensions(path).map_err(|e| format!("读取图片尺寸失败: {e}"))?;
    Ok((w, h))
}

/// 清理本次进程之前留下的临时文件（超过 12 小时的）。
pub fn cleanup_old_temp() {
    let d = temp_dir();
    let Ok(entries) = std::fs::read_dir(d) else { return };
    let now = stamp();
    let max_age_ms = 12u128 * 3600 * 1000;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(modified) = meta.modified() else { continue };
        let age = modified
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .map(|t| now.saturating_sub(t))
            .unwrap_or(0);
        if age > max_age_ms {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
