use serde::Serialize;

use crate::util;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLine {
    pub text: String,
    /// 以下均为图片像素坐标，原点在左上角
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub engine: String,
    pub lines: Vec<OcrLine>,
}

/// 调用系统原生 OCR：macOS 用 Vision，Windows 用 Windows.Media.Ocr。
/// 两者都不需要下载模型，也不联网。
pub fn recognize(path: &str, langs: &str) -> Result<OcrResult, String> {
    let (w, h) = util::image_size(path)?;

    #[cfg(target_os = "macos")]
    {
        return macos::recognize(path, langs, w, h);
    }

    #[cfg(target_os = "windows")]
    {
        return windows::recognize(path, langs, w, h);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (w, h, langs);
        return Err("当前平台暂不支持本地 OCR".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 需要一张真实带文字的 PNG：
    /// `SHOTLY_TEST_IMAGE=/tmp/x.png cargo test ocr_smoke -- --nocapture`
    #[test]
    fn ocr_smoke() {
        let Ok(path) = std::env::var("SHOTLY_TEST_IMAGE") else {
            eprintln!("跳过：未设置 SHOTLY_TEST_IMAGE");
            return;
        };
        let res = recognize(&path, "zh-Hans,en-US").expect("OCR 失败");
        println!("engine = {}", res.engine);
        for l in &res.lines {
            println!(
                "[{:.0},{:.0} {:.0}x{:.0}] {}",
                l.x, l.y, l.width, l.height, l.text
            );
        }
        assert!(!res.lines.is_empty(), "应该至少识别到一行文字");
    }
}

/* ============================== macOS / Vision ============================== */

#[cfg(target_os = "macos")]
mod macos {
    use super::{OcrLine, OcrResult};
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::ptr;

    // 强制链接 Vision 框架，否则运行时找不到 VN* 类
    #[link(name = "Vision", kind = "framework")]
    extern "C" {}

    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn msg_id(obj: *mut c_void, op: *mut c_void, ...) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn msg_void(obj: *mut c_void, op: *mut c_void, ...);
        #[link_name = "objc_msgSend"]
        fn msg_bool(obj: *mut c_void, op: *mut c_void, ...) -> i8;
        #[link_name = "objc_msgSend"]
        fn msg_usize(obj: *mut c_void, op: *mut c_void, ...) -> usize;
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[cfg(target_arch = "x86_64")]
    extern "C" {
        // x86_64 上返回结构体的消息必须走 objc_msgSend_stret
        #[link_name = "objc_msgSend_stret"]
        fn msg_rect(ret: *mut CGRect, obj: *mut c_void, op: *mut c_void, ...);
    }
    #[cfg(not(target_arch = "x86_64"))]
    extern "C" {
        #[link_name = "objc_msgSend"]
        fn msg_rect(obj: *mut c_void, op: *mut c_void, ...) -> CGRect;
    }

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap_or_default()
    }
    unsafe fn class(name: &str) -> *mut c_void {
        let n = cstr(name);
        objc_getClass(n.as_ptr())
    }
    unsafe fn selector(name: &str) -> *mut c_void {
        let n = cstr(name);
        sel_registerName(n.as_ptr())
    }
    unsafe fn nsstr(s: &str) -> *mut c_void {
        let c = cstr(s);
        let cls = class("NSString");
        let alloc = msg_id(cls, selector("alloc"));
        msg_id(alloc, selector("initWithUTF8String:"), c.as_ptr())
    }
    unsafe fn to_string(ns: *mut c_void) -> String {
        if ns.is_null() {
            return String::new();
        }
        let p: *const c_char = msg_id(ns, selector("UTF8String")) as *const c_char;
        if p.is_null() {
            return String::new();
        }
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
    unsafe fn responds(obj: *mut c_void, sel_name: &str) -> bool {
        msg_bool(obj, selector("respondsToSelector:"), selector(sel_name)) != 0
    }
    unsafe fn bbox(obs: *mut c_void) -> CGRect {
        #[cfg(target_arch = "x86_64")]
        {
            let mut r = CGRect::default();
            msg_rect(&mut r, obs, selector("boundingBox"));
            r
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            msg_rect(obs, selector("boundingBox"))
        }
    }

    pub fn recognize(path: &str, langs: &str, img_w: u32, img_h: u32) -> Result<OcrResult, String> {
        unsafe {
            let handler_cls = class("VNImageRequestHandler");
            if handler_cls.is_null() {
                return Err("未找到 Vision 框架（需要 macOS 10.15+）".into());
            }

            let data_cls = class("NSData");
            let cpath = cstr(path);
            let ns_path = nsstr(path);

            // 优先 initWithData:，其次 initWithURL:
            let alloc_h = msg_id(handler_cls, selector("alloc"));
            let handler: *mut c_void = if responds(alloc_h, "initWithData:options:") {
                let data = msg_id(data_cls, selector("dataWithContentsOfFile:"), ns_path);
                if data.is_null() {
                    return Err("读取图片失败".into());
                }
                msg_id(alloc_h, selector("initWithData:options:"), data, ptr::null_mut::<c_void>())
            } else if responds(alloc_h, "initWithURL:options:") {
                let url_cls = class("NSURL");
                let url = msg_id(
                    msg_id(url_cls, selector("alloc")),
                    selector("initFileURLWithPath:"),
                    ns_path,
                );
                msg_id(alloc_h, selector("initWithURL:options:"), url, ptr::null_mut::<c_void>())
            } else {
                return Err("当前系统版本不支持 Vision 图像请求".into());
            };
            let _ = cpath;
            if handler.is_null() {
                return Err("创建 OCR 请求失败".into());
            }

            let req_cls = class("VNRecognizeTextRequest");
            let req = msg_id(msg_id(req_cls, selector("alloc")), selector("init"));
            if req.is_null() {
                return Err("创建文字识别请求失败".into());
            }
            // VNRequestTextRecognitionLevelAccurate = 0
            msg_void(req, selector("setRecognitionLevel:"), 0i64);
            // 变参调用里 BOOL 会被提升为 int，所以这里传 i32 而不是 i8
            msg_void(req, selector("setUsesLanguageCorrection:"), 1i32);

            let arr = msg_id(msg_id(class("NSMutableArray"), selector("alloc")), selector("init"));
            for lang in langs.split(',') {
                let l = lang.trim();
                if l.is_empty() {
                    continue;
                }
                msg_void(arr, selector("addObject:"), nsstr(l));
            }
            if msg_usize(arr, selector("count")) > 0 {
                msg_void(req, selector("setRecognitionLanguages:"), arr);
            }

            let reqs = msg_id(msg_id(class("NSMutableArray"), selector("alloc")), selector("init"));
            msg_void(reqs, selector("addObject:"), req);

            let mut err: *mut c_void = ptr::null_mut();
            let ok = msg_bool(handler, selector("performRequests:error:"), reqs, &mut err);
            if ok == 0 {
                let msg = if err.is_null() {
                    "识别失败".to_string()
                } else {
                    to_string(msg_id(err, selector("localizedDescription")))
                };
                return Err(msg);
            }

            let results = msg_id(req, selector("results"));
            let count = if results.is_null() { 0 } else { msg_usize(results, selector("count")) };
            let mut lines = Vec::with_capacity(count);
            for i in 0..count {
                let obs = msg_id(results, selector("objectAtIndex:"), i);
                let cands = msg_id(obs, selector("topCandidates:"), 1usize);
                if cands.is_null() || msg_usize(cands, selector("count")) == 0 {
                    continue;
                }
                let top = msg_id(cands, selector("objectAtIndex:"), 0usize);
                let text = to_string(msg_id(top, selector("string")));
                if text.trim().is_empty() {
                    continue;
                }
                let r = bbox(obs);
                lines.push(OcrLine {
                    text,
                    // Vision 的 boundingBox 是归一化坐标，原点在左下角
                    x: r.origin.x * img_w as f64,
                    y: (1.0 - r.origin.y - r.size.height) * img_h as f64,
                    width: r.size.width * img_w as f64,
                    height: r.size.height * img_h as f64,
                });
            }

            Ok(OcrResult { engine: "Apple Vision".into(), lines })
        }
    }
}

/* ========================= Windows / Windows.Media.Ocr ========================= */

#[cfg(target_os = "windows")]
mod windows {
    use super::{OcrLine, OcrResult};
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    pub fn recognize(path: &str, langs: &str, img_w: u32, img_h: u32) -> Result<OcrResult, String> {
        let img = image::open(path).map_err(|e| format!("打开图片失败: {e}"))?;
        let rgba = img.to_rgba8();
        let (w, h) = (img_w.max(rgba.width()), img_h.max(rgba.height()));
        let src = rgba.into_raw();

        // Windows OCR 需要 BGRA8 预乘
        let mut bgra = vec![0u8; (w as usize) * (h as usize) * 4];
        let row_bytes = (w as usize) * 4;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let i = (y * row_bytes + x * 4) as usize;
                let o = (y * row_bytes + x * 4) as usize;
                let (r, g, b, a) = if i + 3 < src.len() {
                    (src[i], src[i + 1], src[i + 2], src[i + 3])
                } else {
                    (0, 0, 0, 255)
                };
                let af = a as f32 / 255.0;
                bgra[o] = (b as f32 * af) as u8;
                bgra[o + 1] = (g as f32 * af) as u8;
                bgra[o + 2] = (r as f32 * af) as u8;
                bgra[o + 3] = a;
            }
        }

        let stream = InMemoryRandomAccessStream::new().map_err(|e| e.to_string())?;
        let writer = DataWriter::CreateDataWriter(&stream).map_err(|e| e.to_string())?;
        writer.WriteBytes(&bgra).map_err(|e| e.to_string())?;
        let buffer = writer.DetachBuffer().map_err(|e| e.to_string())?;

        // 注意：CreateCopyFromBuffer 只接受 4 个参数（不含 alpha），
        // 需要指定 alpha 模式要用 CreateCopyWithAlphaFromBuffer
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            w as i32,
            h as i32,
            BitmapAlphaMode::Premultiplied,
        )
        .map_err(|e| e.to_string())?;

        let engine = pick_engine(langs)?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e| e.to_string())?
            .get()
            .map_err(|e| e.to_string())?;

        let ocr_lines = result.Lines().map_err(|e| e.to_string())?;
        let count = ocr_lines.Size().map_err(|e| e.to_string())? as usize;
        let mut lines = Vec::new();
        for i in 0..count {
            let line = ocr_lines.GetAt(i as u32).map_err(|e| e.to_string())?;
            let text = line.Text().map_err(|e| e.to_string())?.to_string();
            if text.trim().is_empty() {
                continue;
            }
            // OcrLine 没有直接的包围盒，用词级包围盒取并集
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            if let Ok(words) = line.Words() {
                let wc = words.Size().unwrap_or(0) as usize;
                for j in 0..wc {
                    if let Ok(word) = words.GetAt(j as u32) {
                        if let Ok(r) = word.BoundingRect() {
                            x0 = x0.min(r.X as f64);
                            y0 = y0.min(r.Y as f64);
                            x1 = x1.max((r.X + r.Width) as f64);
                            y1 = y1.max((r.Y + r.Height) as f64);
                        }
                    }
                }
            }
            if x0 == f64::MAX {
                x0 = 0.0; y0 = 0.0; x1 = 0.0; y1 = 0.0;
            }
            lines.push(OcrLine {
                text,
                x: x0,
                y: y0,
                width: (x1 - x0).max(0.0),
                height: (y1 - y0).max(0.0),
            });
        }

        Ok(OcrResult { engine: "Windows OCR".into(), lines })
    }

    fn pick_engine(langs: &str) -> Result<OcrEngine, String> {
        for l in langs.split(',') {
            let tag = l.trim();
            if tag.is_empty() {
                continue;
            }
            let tag = match tag {
                "zh-Hans" => "zh-Hans-CN",
                "en-US" => "en-US",
                "ja-JP" => "ja-JP",
                other => other,
            };
            if let Ok(lang) = Language::CreateLanguage(&HSTRING::from(tag)) {
                if let Ok(e) = OcrEngine::TryCreateFromLanguage(&lang) {
                    return Ok(e);
                }
            }
        }
        OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "系统未安装任何 OCR 语言包".to_string())
    }
}
