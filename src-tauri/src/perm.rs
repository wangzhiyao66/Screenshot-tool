use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub granted: bool,
    pub hint: String,
}

#[cfg(target_os = "macos")]
mod imp {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    pub fn granted() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn request() {
        unsafe {
            CGRequestScreenCaptureAccess();
        }
    }

    pub fn hint() -> String {
        "需要「屏幕录制」权限。请在 系统设置 → 隐私与安全性 → 屏幕录制 中勾选 Shotly，\
         然后**完全退出（⌘Q）再重新启动**才生效——只关窗口是没用的。"
            .to_string()
    }
}

#[cfg(target_os = "windows")]
mod imp {
    pub fn granted() -> bool {
        true
    }
    pub fn request() {}
    pub fn hint() -> String {
        "Windows 无需额外授权。若无法截取某些窗口，请尝试以管理员身份运行。".to_string()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn granted() -> bool {
        false
    }
    pub fn request() {}
    pub fn hint() -> String {
        "当前平台暂未适配。".to_string()
    }
}

pub fn check() -> Permission {
    let granted = imp::granted();
    if !granted {
        imp::request();
    }
    Permission {
        granted,
        hint: if granted { String::new() } else { imp::hint() },
    }
}
