//! 让本程序自己的窗口也从截屏中排除——反截屏工具自己当然不该出现在截图里。
//!
//! 本进程直接对自己的窗口调用 SetWindowDisplayAffinity，无需注入。
//! 每帧调用一次（开销极小），可覆盖启动后才创建的窗口。

use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetWindowDisplayAffinity,
    WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

thread_local! {
    static AFFINITY: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// 设置本进程所有可见顶层窗口是否从截屏排除。
pub fn set_self_protected(protected: bool) {
    let aff = if protected {
        WDA_EXCLUDEFROMCAPTURE.0
    } else {
        WDA_NONE.0
    };
    AFFINITY.with(|c| c.set(aff));
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(0));
    }
}

unsafe extern "system" fn enum_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == GetCurrentProcessId() && IsWindowVisible(hwnd).as_bool() {
        let aff = AFFINITY.with(|c| c.get());
        let _ = SetWindowDisplayAffinity(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_DISPLAY_AFFINITY(aff),
        );
    }
    BOOL(1)
}
