use windows::Win32::Globalization::GetUserDefaultUILanguage;

#[derive(Clone, Copy)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn detect() -> Self {
        if let Ok(value) = std::env::var("CAPTUREGUARD_LANG") {
            if let Some(lang) = Self::from_locale(&value) {
                return lang;
            }
        }

        if ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|key| std::env::var(key).ok())
            .any(|value| Self::from_locale(&value).is_some_and(|lang| lang.is_chinese()))
        {
            return Self::Chinese;
        }

        // Windows primary language id 0x04 is Chinese.
        let lang_id = unsafe { GetUserDefaultUILanguage() };
        if lang_id & 0x03ff == 0x04 {
            Self::Chinese
        } else {
            Self::English
        }
    }

    fn from_locale(value: &str) -> Option<Self> {
        let normalized = value.to_ascii_lowercase().replace('_', "-");
        if normalized.starts_with("zh") || normalized.contains("chinese") {
            Some(Self::Chinese)
        } else if normalized.starts_with("en") || normalized == "c" || normalized == "posix" {
            Some(Self::English)
        } else {
            None
        }
    }

    fn is_chinese(self) -> bool {
        matches!(self, Self::Chinese)
    }

    pub fn window_title(self) -> &'static str {
        match self {
            Self::English => "CaptureGuard",
            Self::Chinese => "CaptureGuard 截屏防护",
        }
    }

    pub fn heading(self) -> &'static str {
        match self {
            Self::English => "Capture protection",
            Self::Chinese => "截屏防护",
        }
    }

    pub fn refresh_button(self) -> &'static str {
        match self {
            Self::English => "Refresh",
            Self::Chinese => "刷新列表",
        }
    }

    pub fn hide_self_checkbox(self) -> &'static str {
        match self {
            Self::English => "Hide this window",
            Self::Chinese => "隐藏本窗口",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::English => {
                "Protected windows are excluded from screenshots and recordings while staying visible to you. Protection keeps running after this app is closed."
            }
            Self::Chinese => {
                "对选中进程开启防护后，其窗口在截屏/录屏中将被排除（你自己看正常）。注入后可关闭本程序，防护持续生效。"
            }
        }
    }

    pub fn protect_button(self) -> &'static str {
        match self {
            Self::English => "Protect",
            Self::Chinese => "开启防护",
        }
    }

    pub fn unprotect_button(self) -> &'static str {
        match self {
            Self::English => "Unprotect",
            Self::Chinese => "解除防护",
        }
    }

    pub fn initial_status(self) -> &'static str {
        match self {
            Self::English => "Select a process, then click Protect.",
            Self::Chinese => "选择一个进程，点击开启防护。",
        }
    }

    pub fn refreshed_status(self) -> &'static str {
        match self {
            Self::English => "Process list refreshed.",
            Self::Chinese => "已刷新进程列表。",
        }
    }

    pub fn no_processes(self) -> &'static str {
        match self {
            Self::English => "No windowed processes found.",
            Self::Chinese => "未发现有窗口的进程。",
        }
    }

    pub fn component_unavailable(self) -> &'static str {
        match self {
            Self::English => "Protection component unavailable.",
            Self::Chinese => "防护组件不可用。",
        }
    }

    pub fn protected_mark(self) -> &'static str {
        match self {
            Self::English => "Protected",
            Self::Chinese => "已防护",
        }
    }

    pub fn unprotected_mark(self) -> &'static str {
        match self {
            Self::English => "Not protected",
            Self::Chinese => "未防护",
        }
    }

    pub fn injected_but_not_excluded(self, name: &str) -> String {
        match self {
            Self::English => format!(
                "Injected into {name}, but no window was detected as excluded. Security software may have blocked it, or the visible window may belong to another process such as UWP/ApplicationFrameHost or a multi-process browser. Recordings may still capture it."
            ),
            Self::Chinese => format!(
                "已注入 {name}，但未检测到窗口被排除。可能被杀软拦截，或其窗口属于其它进程（如 UWP/部分浏览器）。录屏可能仍可见。"
            ),
        }
    }

    pub fn protect_success(self, name: &str, pid: u32, excluded: u32, total: u32) -> String {
        match self {
            Self::English => format!(
                "Protection enabled for {name} (PID {pid}). {excluded}/{total} windows are excluded."
            ),
            Self::Chinese => {
                format!("已对 {name} (PID {pid}) 开启防护（{excluded}/{total} 窗口已排除）。")
            }
        }
    }

    pub fn protect_failed(self, error: &str) -> String {
        match self {
            Self::English => {
                format!("Protection failed: {error}. Administrator privileges may be required, or security software may have blocked injection.")
            }
            Self::Chinese => format!("开启失败：{error}（可能需管理员权限或被杀软拦截）"),
        }
    }

    pub fn unprotect_requested(self, name: &str, pid: u32) -> String {
        match self {
            Self::English => format!("Unprotect requested for {name} (PID {pid})."),
            Self::Chinese => format!("已请求解除 {name} (PID {pid}) 的防护。"),
        }
    }

    pub fn unprotect_failed(self, error: &str) -> String {
        match self {
            Self::English => format!("Unprotect failed: {error}"),
            Self::Chinese => format!("解除失败：{error}"),
        }
    }

    pub fn selftest_invalid_pid(self) -> &'static str {
        match self {
            Self::English => "pid must be numeric",
            Self::Chinese => "pid 必须是数字",
        }
    }

    pub fn selftest_extract_failed(self, error: &str) -> String {
        match self {
            Self::English => format!("Failed to extract embedded DLL: {error}"),
            Self::Chinese => format!("释放内嵌 DLL 失败: {error}"),
        }
    }

    pub fn selftest_extracted_to(self, path: &str) -> String {
        match self {
            Self::English => format!("Embedded DLL extracted to: {path}"),
            Self::Chinese => format!("内嵌 DLL 已释放到: {path}"),
        }
    }

    pub fn selftest_inject_success(self) -> &'static str {
        match self {
            Self::English => "Injection succeeded",
            Self::Chinese => "注入成功",
        }
    }

    pub fn selftest_inject_failed(self, error: &str) -> String {
        match self {
            Self::English => format!("Injection failed: {error}"),
            Self::Chinese => format!("注入失败: {error}"),
        }
    }

    pub fn selftest_window_stats(self, excluded: u32, total: u32) -> String {
        match self {
            Self::English => {
                format!("Window check: {excluded}/{total} visible top-level windows are excluded.")
            }
            Self::Chinese => format!("窗口校验：{excluded}/{total} 个可见顶层窗口已设为截屏排除"),
        }
    }

    pub fn selftest_no_effect(self) -> &'static str {
        match self {
            Self::English => {
                "Injection may not have taken effect. It may have been blocked, or the visible window may belong to another process such as UWP/ApplicationFrameHost."
            }
            Self::Chinese => {
                "注入似乎未生效（可能被拦截，或目标窗口属于其它进程，如 UWP 的 ApplicationFrameHost）"
            }
        }
    }

    pub fn selftest_unprotect_requested(self) -> &'static str {
        match self {
            Self::English => "Unprotect requested",
            Self::Chinese => "已请求解除",
        }
    }

    pub fn selftest_unprotect_failed(self, error: &str) -> String {
        match self {
            Self::English => format!("Unprotect failed: {error}"),
            Self::Chinese => format!("解除失败: {error}"),
        }
    }
}
