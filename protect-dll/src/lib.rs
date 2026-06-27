//! protect-dll：注入目标进程后，把它的窗口设为"截屏排除"。
//!
//! 自维持设计：注入后这个 DLL 在目标进程内独立运行，**不依赖**外部 GUI 挂着。
//! GUI 注入完即可关闭，DLL 继续工作。
//!
//! 原理：`SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` 由 DWM 在合成层
//! 把窗口从所有截屏/录屏中排除（PrintScreen / BitBlt / Win+Shift+S / DXGI / OBS
//! 全部抓不到），但用户本人看屏幕正常。该 API 只能由窗口所属进程调用，故走注入。
//!
//! 卸载握手：DLL 在目标内创建命名事件 `Local\CaptureGuardUnload_<PID>`。
//! 后台线程平时每 250ms 给可见顶层窗口（及其子窗口）补设排除属性，同时等待该事件。
//! GUI（或任何进程）下次想解除时，打开同名事件并 SetEvent；DLL 收到后把窗口
//! 还原为 WDA_NONE，然后 FreeLibraryAndExitThread 自卸载——目标进程恢复原状。

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, FALSE, HANDLE, HINSTANCE, HWND, LPARAM, TRUE};
use windows::Win32::System::LibraryLoader::FreeLibraryAndExitThread;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::System::Threading::{CreateEventW, GetCurrentProcessId, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_MONITOR, WDA_NONE,
    WINDOW_DISPLAY_AFFINITY,
};

static STARTED: AtomicBool = AtomicBool::new(false);
/// 保存本 DLL 的模块句柄，供 FreeLibraryAndExitThread 自卸载用。
static HMODULE_SELF: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

/// DLL 入口。attach 时记录自身句柄并起后台线程；detach 不做事（卸载走事件握手）。
#[no_mangle]
#[allow(non_snake_case, clippy::missing_safety_doc)]
pub extern "system" fn DllMain(
    hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            if !STARTED.swap(true, Ordering::SeqCst) {
                HMODULE_SELF.store(hinst.0, Ordering::SeqCst);
                std::thread::spawn(worker);
            }
        }
        DLL_PROCESS_DETACH => {}
        _ => {}
    }
    BOOL(1)
}

/// 拼出本进程对应的卸载事件名（带 NUL 结尾的宽字符串）。
/// 前缀用 obfstr 混淆，避免在二进制里以明文出现——否则任何人看到事件名
/// 就能 SetEvent 解除防护。运行时才解密。
fn unload_event_name() -> Vec<u16> {
    let pid = unsafe { GetCurrentProcessId() };
    format!("{}{pid}", obfstr::obfstr!(r"Local\CaptureGuardUnload_"))
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// 后台工作线程：循环设排除 + 等待卸载事件。
fn worker() {
    // 手动重置事件，初始未触发。
    let name = unload_event_name();
    let event = unsafe { CreateEventW(None, TRUE, FALSE, PCWSTR(name.as_ptr())) };
    let event: HANDLE = match event {
        Ok(h) => h,
        // 事件建不出来就退化为纯循环（仍能防截屏，只是无法被远程解除）。
        Err(_) => loop {
            apply_to_all_windows(WDA_EXCLUDEFROMCAPTURE.0);
            std::thread::sleep(std::time::Duration::from_millis(250));
        },
    };

    loop {
        // 每轮先把所有可见顶层窗口设为排除（覆盖新建窗口）。
        apply_to_all_windows(WDA_EXCLUDEFROMCAPTURE.0);

        // 等卸载事件，超时 250ms 就再刷一轮（兼顾新窗口覆盖与解除响应速度）。
        let r = unsafe { WaitForSingleObject(event, 250) };
        // WAIT_OBJECT_0 == 0：事件被触发，执行卸载。
        if r.0 == 0 {
            break;
        }
    }

    // 还原所有窗口为正常（可被截屏），然后自卸载。
    apply_to_all_windows(WDA_NONE.0);

    // 关闭事件句柄——这是最后一个引用，关掉后命名对象消失，
    // GUI 再 OpenEvent 会失败，从而正确判定为"未防护"。
    unsafe {
        let _ = CloseHandle(event);
    }

    let hmod = HMODULE_SELF.load(Ordering::SeqCst);
    unsafe {
        FreeLibraryAndExitThread(HINSTANCE(hmod), 0);
    }
}

thread_local! {
    /// EnumWindows/EnumChildWindows 回调通过它读取本轮要设置的 affinity 值。
    static AFFINITY: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// 给本进程所有可见顶层窗口（及其子窗口）设置指定 affinity。
///
/// 为什么要连子窗口一起设：很多程序（Chromium/CEF、Qt、游戏等）真正绘制内容的是
/// 子窗口；而下拉框、工具提示等是独立的顶层弹出窗口。只设主窗口往往漏掉这些表面，
/// 导致录屏仍能拍到部分内容。这里对顶层窗口本身 + 其所有子窗口都设一遍。
fn apply_to_all_windows(affinity: u32) {
    AFFINITY.with(|c| c.set(affinity));
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(0));
    }
}

/// 给单个窗口设置 affinity（忽略失败：部分窗口类型不支持，属正常）。
///
/// 对"排除截屏"做兜底：`WDA_EXCLUDEFROMCAPTURE` 需要 Windows 10 2004(19041)+，
/// 旧系统上该值会被拒绝。此时退化为 `WDA_MONITOR`——截屏/录屏里窗口显示为黑块，
/// 虽不如完全隐形，但内容不外泄，强于原样可见。
unsafe fn set_affinity(hwnd: HWND, affinity: u32) {
    if SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(affinity)).is_err()
        && affinity == WDA_EXCLUDEFROMCAPTURE.0
    {
        let _ = SetWindowDisplayAffinity(hwnd, WDA_MONITOR);
    }
}

/// EnumWindows 回调：处理属于本进程的可见顶层窗口，并递归其子窗口。
unsafe extern "system" fn enum_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));

    if pid == GetCurrentProcessId() && IsWindowVisible(hwnd).as_bool() {
        let aff = AFFINITY.with(|c| c.get());
        set_affinity(hwnd, aff);
        // 递归子窗口：内容/渲染表面常在子窗口里。
        let _ = EnumChildWindows(hwnd, Some(enum_child_proc), LPARAM(aff as isize));
    }
    BOOL(1) // 继续枚举
}

/// EnumChildWindows 回调：给每个子窗口也设上同样的 affinity。
unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    set_affinity(hwnd, lparam.0 as u32);
    BOOL(1)
}
