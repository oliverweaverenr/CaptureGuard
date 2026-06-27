//! Optional capture exclusion for CaptureGuard's own window.
//!
//! This process can call SetWindowDisplayAffinity on its own windows directly,
//! without injection. The call is repeated every frame to cover late-created
//! windows.

use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetWindowDisplayAffinity,
    WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

thread_local! {
    static AFFINITY: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Set whether all visible top-level windows in this process are capture-excluded.
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
