//! 注入 / 解除 / 状态检测。
//!
//! - inject: 经典 LoadLibraryW 远程线程注入。
//! - is_protected: 尝试打开 `Local\CaptureGuardUnload_<PID>` 事件，打得开说明
//!   该进程已被注入（DLL 正在运行并持有该事件）。
//! - request_unprotect: 打开同名事件并 SetEvent，DLL 收到后自还原 + 自卸载。

use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, FALSE, HWND, LPARAM};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, OpenEventW, OpenProcess, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    LPTHREAD_START_ROUTINE, PROCESS_ALL_ACCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsWindowVisible,
    WDA_EXCLUDEFROMCAPTURE,
};

/// 拼出某 PID 的卸载事件名。前缀必须与 protect-dll 端一致，
/// 用 obfstr 混淆避免明文暴露（明文事件名 = 任何人都能解除防护）。
fn event_name(pid: u32) -> Vec<u16> {
    format!("{}{pid}", obfstr::obfstr!(r"Local\CaptureGuardUnload_"))
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// 该进程是否已被注入（通过能否打开卸载事件判断）。
pub fn is_protected(pid: u32) -> bool {
    let name = event_name(pid);
    unsafe {
        match OpenEventW(EVENT_MODIFY_STATE, FALSE, PCWSTR(name.as_ptr())) {
            Ok(h) if !h.is_invalid() => {
                let _ = CloseHandle(h);
                true
            }
            _ => false,
        }
    }
}

/// 请求解除：触发卸载事件，DLL 会还原窗口并自卸载。
pub fn request_unprotect(pid: u32) -> Result<(), String> {
    let name = event_name(pid);
    unsafe {
        let h = OpenEventW(EVENT_MODIFY_STATE, FALSE, PCWSTR(name.as_ptr()))
            .map_err(|e| format!("打开卸载事件失败（可能未注入）: {e}"))?;
        let r = SetEvent(h);
        let _ = CloseHandle(h);
        r.map_err(|e| format!("触发卸载事件失败: {e}"))
    }
}

/// 注入 protect_dll.dll 到指定进程。
pub fn inject(pid: u32, dll_path: &Path) -> Result<(), String> {
    let dll_abs = dll_path
        .canonicalize()
        .map_err(|e| format!("解析 DLL 路径失败: {e}"))?;
    let dll_str = dll_abs.to_string_lossy().to_string();

    unsafe {
        let process = OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid)
            .map_err(|e| format!("OpenProcess 失败: {e}"))?;

        let mut wide: Vec<u16> = dll_str.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = wide.len() * 2;

        let remote_mem = VirtualAllocEx(
            process,
            None,
            byte_len,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if remote_mem.is_null() {
            let _ = CloseHandle(process);
            return Err("VirtualAllocEx 失败".into());
        }

        let write_ok = WriteProcessMemory(
            process,
            remote_mem,
            wide.as_mut_ptr() as *const _,
            byte_len,
            None,
        );
        if write_ok.is_err() {
            let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
            let _ = CloseHandle(process);
            return Err("WriteProcessMemory 失败".into());
        }

        let load_library =
            get_proc("kernel32.dll", "LoadLibraryW").ok_or("无法定位 LoadLibraryW")?;
        let start: LPTHREAD_START_ROUTINE = Some(std::mem::transmute::<
            *const core::ffi::c_void,
            unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
        >(load_library));

        let thread = CreateRemoteThread(process, None, 0, start, Some(remote_mem), 0, None)
            .map_err(|e| {
                let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                format!("CreateRemoteThread 失败: {e}")
            })?;

        let _ = WaitForSingleObject(thread, 10_000);
        let _ = CloseHandle(thread);
        let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
        let _ = CloseHandle(process);
        Ok(())
    }
}

/// 取本进程中 module!func 的地址（系统 DLL 各进程同址）。
unsafe fn get_proc(module: &str, func: &str) -> Option<*const core::ffi::c_void> {
    let m: Vec<u16> = module.encode_utf16().chain(std::iter::once(0)).collect();
    let f: Vec<u8> = func.bytes().chain(std::iter::once(0)).collect();
    let hmod = GetModuleHandleW(PCWSTR(m.as_ptr())).ok()?;
    GetProcAddress(hmod, windows::core::PCSTR(f.as_ptr())).map(|p| p as *const core::ffi::c_void)
}

/// 校验防护是否真的生效：跨进程读取目标可见顶层窗口的截屏排除属性。
///
/// `GetWindowDisplayAffinity` 可读任意窗口（无需拥有），用它确认 DLL 是否把窗口
/// 设成了 `WDA_EXCLUDEFROMCAPTURE`。返回 `(已排除窗口数, 可见顶层窗口总数)`：
/// 若总数 > 0 而已排除数为 0，多半是注入没成功或目标窗口属于别的进程（如 UWP 的
/// ApplicationFrameHost）。主要用于自检与排障。
pub fn protected_window_stats(pid: u32) -> (u32, u32) {
    struct Ctx {
        pid: u32,
        excluded: u32,
        total: u32,
    }
    let mut ctx = Ctx {
        pid,
        excluded: 0,
        total: 0,
    };
    unsafe {
        let _ = EnumWindows(Some(count_proc), LPARAM(&mut ctx as *mut Ctx as isize));
    }

    unsafe extern "system" fn count_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        let mut wpid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut wpid));
        if wpid == ctx.pid && IsWindowVisible(hwnd).as_bool() {
            ctx.total += 1;
            let mut aff = 0u32;
            if GetWindowDisplayAffinity(hwnd, &mut aff).is_ok() && aff == WDA_EXCLUDEFROMCAPTURE.0 {
                ctx.excluded += 1;
            }
        }
        BOOL(1)
    }

    (ctx.excluded, ctx.total)
}
