//! 窗口 / 控件智能识别。
//!
//! 思路：
//! 1. 通过 Core Graphics 的 `CGWindowListCopyWindowInfo` 枚举屏幕上所有窗口，
//!    过滤掉本进程（Shotly 自身的覆盖层 / 设置 / 编辑器）后，取光标所在的最上层
//!    窗口作为「窗口级」识别结果。
//! 2. 若开启了「控件级」且本进程已获得辅助功能授权，则在该窗口所属 App 的
//!    AXUIElement 树上做命中测试（`AXUIElementCopyElementAtPosition`），拿到光标下
//!    最深的元素（按钮、输入框等）的几何与角色，作为「控件级」结果。
//! 3. 所有坐标均以「全局逻辑点」返回（原点在主板左上角，y 轴向下），与前端覆盖层
//!    的坐标系一致。

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedRect {
    /// 全局逻辑点 x（相对于主屏左上角）
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// 可读标题（窗口标题 / 控件标题）
    pub title: String,
    /// 角色：window / AXButton / AXTextField ...
    pub role: String,
}

pub fn detect_element(x: f64, y: f64, level: &str) -> Option<DetectedRect> {
    #[cfg(target_os = "macos")]
    {
        imp::detect(x, y, level)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (x, y, level);
        None
    }
}

pub fn ax_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        imp::ax_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::DetectedRect;
    use std::os::raw::{c_char, c_int, c_void};
    use std::process;

    type CFIndex = std::os::raw::c_long;
    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFArrayRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFNumberRef = *const c_void;
    type AXUIElementRef = *const c_void;
    type AXValueRef = *const c_void;

    const UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_DOUBLE: i32 = 13; // kCFNumberDoubleType
    const AX_VALUE_CGPOINT: i32 = 1; // kAXValueCGPointType
    const AX_VALUE_CGSIZE: i32 = 2; // kAXValueCGSizeType
    const WINDOW_LIST_OPTION: u32 = 0x1 | 0x10; // kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements

    #[link(name = "CoreFoundation", kind = "framework")]
    #[link(name = "CoreGraphics", kind = "framework")]
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFTypeRef);
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            cstr: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFStringGetCString(
            string: CFStringRef,
            buffer: *mut c_char,
            buf_size: CFIndex,
            encoding: u32,
        ) -> u8;
        fn CFArrayGetCount(arr: CFArrayRef) -> CFIndex;
        fn CFArrayGetValueAtIndex(arr: CFArrayRef, idx: CFIndex) -> CFTypeRef;
        fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFStringRef) -> CFTypeRef;
        fn CFNumberGetValue(num: CFNumberRef, the_type: i32, value_ptr: *mut c_void) -> u8;
        fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;

        fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
        fn AXUIElementCopyElementAtPosition(
            element: AXUIElementRef,
            x: f64,
            y: f64,
            out: *mut AXUIElementRef,
        ) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            out: *mut CFTypeRef,
        ) -> i32;
        fn AXValueGetValue(value: AXValueRef, the_type: i32, out: *mut c_void) -> u8;
        fn AXIsProcessTrusted() -> u8;
    }

    fn make_cfstring(s: &str) -> CFStringRef {
        unsafe {
            CFStringCreateWithCString(std::ptr::null(), s.as_ptr() as *const c_char, UTF8)
        }
    }

    fn cfstring_to_string(s: CFStringRef) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let mut buf = [0i8; 1024];
        let ok = unsafe {
            CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as CFIndex, UTF8)
        };
        if ok == 0 {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let bytes: Vec<u8> = buf[..end].iter().map(|&c| c as u8).collect();
        String::from_utf8(bytes).ok()
    }

    fn number_to_f64(num: CFTypeRef) -> Option<f64> {
        if num.is_null() {
            return None;
        }
        let mut v: f64 = 0.0;
        let ok = unsafe {
            CFNumberGetValue(
                num as CFNumberRef,
                K_CF_NUMBER_DOUBLE,
                &mut v as *mut f64 as *mut c_void,
            )
        };
        if ok == 0 {
            return None;
        }
        Some(v)
    }

    fn dict_number(dict: CFDictionaryRef, key: &str) -> Option<f64> {
        let k = make_cfstring(key);
        let v = unsafe { CFDictionaryGetValue(dict, k) };
        unsafe { CFRelease(k) };
        number_to_f64(v)
    }

    fn dict_string(dict: CFDictionaryRef, key: &str) -> Option<String> {
        let k = make_cfstring(key);
        let v = unsafe { CFDictionaryGetValue(dict, k) };
        unsafe { CFRelease(k) };
        cfstring_to_string(v as CFStringRef)
    }

    fn window_bounds(dict: CFDictionaryRef) -> Option<(f64, f64, f64, f64)> {
        let k = make_cfstring("Bounds");
        let v = unsafe { CFDictionaryGetValue(dict, k) } as CFDictionaryRef;
        unsafe { CFRelease(k) };
        if v.is_null() {
            return None;
        }
        let x = dict_number(v, "X")?;
        let y = dict_number(v, "Y")?;
        let w = dict_number(v, "Width")?;
        let h = dict_number(v, "Height")?;
        Some((x, y, w, h))
    }

    fn ax_attr_point(el: AXUIElementRef, attr: &str, ax_type: i32) -> Option<(f64, f64)> {
        let k = make_cfstring(attr);
        let mut val: CFTypeRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(el, k, &mut val) };
        unsafe { CFRelease(k) };
        if err != 0 || val.is_null() {
            return None;
        }
        let mut pt = (0.0f64, 0.0f64);
        let ok = unsafe {
            AXValueGetValue(
                val as AXValueRef,
                ax_type,
                &mut pt as *mut (f64, f64) as *mut c_void,
            )
        };
        unsafe { CFRelease(val) };
        if ok == 0 {
            return None;
        }
        Some(pt)
    }

    fn ax_attr_string(el: AXUIElementRef, attr: &str) -> Option<String> {
        let k = make_cfstring(attr);
        let mut val: CFTypeRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(el, k, &mut val) };
        unsafe { CFRelease(k) };
        if err != 0 || val.is_null() {
            return None;
        }
        let s = cfstring_to_string(val as CFStringRef);
        unsafe { CFRelease(val) };
        s
    }

    /// 在指定 App 的 AX 树上对全局坐标 (gx, gy) 做命中测试，返回最深元素的几何与角色。
    fn ax_element_at(
        pid: c_int,
        gx: f64,
        gy: f64,
    ) -> Option<(f64, f64, f64, f64, String, String)> {
        let app = unsafe { AXUIElementCreateApplication(pid) };
        if app.is_null() {
            return None;
        }
        let mut elem: AXUIElementRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyElementAtPosition(app, gx, gy, &mut elem) };
        let result = if err == 0 && !elem.is_null() {
            let pos = ax_attr_point(elem, "AXPosition", AX_VALUE_CGPOINT);
            let size = ax_attr_point(elem, "AXSize", AX_VALUE_CGSIZE);
            let role = ax_attr_string(elem, "AXRole").unwrap_or_default();
            let title = ax_attr_string(elem, "AXTitle").unwrap_or_default();
            match (pos, size) {
                (Some((px, py)), Some((sw, sh))) => {
                    Some((px, py, sw, sh, role, title))
                }
                _ => None,
            }
        } else {
            None
        };
        unsafe {
            if !elem.is_null() {
                CFRelease(elem);
            }
            CFRelease(app);
        }
        result
    }

    pub fn ax_trusted() -> bool {
        unsafe { AXIsProcessTrusted() != 0 }
    }

    pub fn detect(x: f64, y: f64, level: &str) -> Option<DetectedRect> {
        let array = unsafe { CGWindowListCopyWindowInfo(WINDOW_LIST_OPTION, 0) };
        if array.is_null() {
            return None;
        }
        let count = unsafe { CFArrayGetCount(array) };
        let self_pid = process::id() as f64;

        // (x, y, w, h, ownerPID, title)
        let mut found: Option<(f64, f64, f64, f64, f64, String)> = None;
        for i in 0..count {
            let dict = unsafe { CFArrayGetValueAtIndex(array, i) } as CFDictionaryRef;
            if dict.is_null() {
                continue;
            }
            let owner_pid = dict_number(dict, "OwnerPID").unwrap_or(-1.0);
            // 跳过 Shotly 自己的窗口（覆盖层 / 设置 / 编辑器 / 固定）
            if (owner_pid - self_pid).abs() < 0.5 {
                continue;
            }
            let (bx, by, bw, bh) = match window_bounds(dict) {
                Some(b) => b,
                None => continue,
            };
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                let name = dict_string(dict, "Name").unwrap_or_default();
                let owner = dict_string(dict, "OwnerName").unwrap_or_default();
                let label = if name.is_empty() { owner } else { name };
                found = Some((bx, by, bw, bh, owner_pid, label));
                break; // 数组按前后顺序返回，第一个命中即最上层
            }
        }
        unsafe { CFRelease(array) };

        let (wx, wy, ww, wh, owner_pid, wtitle) = found?;

        let mut rect = DetectedRect {
            x: wx,
            y: wy,
            w: ww,
            h: wh,
            title: wtitle,
            role: "window".into(),
        };

        if level == "control" && ax_trusted() {
            if let Some((cx, cy, cw, ch, crole, ctitle)) =
                ax_element_at(owner_pid as c_int, x, y)
            {
                if cw > 2.0 && ch > 2.0 {
                    rect = DetectedRect {
                        x: cx,
                        y: cy,
                        w: cw,
                        h: ch,
                        title: if ctitle.is_empty() {
                            rect.title.clone()
                        } else {
                            ctitle
                        },
                        role: crole,
                    };
                }
            }
        }

        Some(rect)
    }
}
