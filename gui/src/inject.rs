//! Injection, unprotect, and status checks.
//!
//! - inject: classic LoadLibraryW remote-thread injection.
//! - is_protected: opens `Local\CaptureGuardUnload_<PID>`; success means the DLL
//!   is running in the target process and owns the event.
//! - request_unprotect: signals the event so the DLL restores windows and unloads.

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

/// Build the unload event name for a PID. The prefix must match protect-dll.
/// obfstr avoids exposing the plain event name in the binary.
fn event_name(pid: u32) -> Vec<u16> {
    format!("{}{pid}", obfstr::obfstr!(r"Local\CaptureGuardUnload_"))
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// Whether this process is currently protected, detected through the unload event.
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

/// Request unprotect by signaling the unload event.
pub fn request_unprotect(pid: u32) -> Result<(), String> {
    let name = event_name(pid);
    unsafe {
        let h = OpenEventW(EVENT_MODIFY_STATE, FALSE, PCWSTR(name.as_ptr())).map_err(|e| {
            format!("failed to open unload event; process may not be protected: {e}")
        })?;
        let r = SetEvent(h);
        let _ = CloseHandle(h);
        r.map_err(|e| format!("failed to signal unload event: {e}"))
    }
}

/// Inject protect_dll.dll into the target process.
pub fn inject(pid: u32, dll_path: &Path) -> Result<(), String> {
    let dll_abs = dll_path
        .canonicalize()
        .map_err(|e| format!("failed to resolve DLL path: {e}"))?;
    let dll_str = dll_abs.to_string_lossy().to_string();

    unsafe {
        let process = OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid)
            .map_err(|e| format!("OpenProcess failed: {e}"))?;

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
            return Err("VirtualAllocEx failed".into());
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
            return Err("WriteProcessMemory failed".into());
        }

        let load_library =
            get_proc("kernel32.dll", "LoadLibraryW").ok_or("failed to locate LoadLibraryW")?;
        let start: LPTHREAD_START_ROUTINE = Some(std::mem::transmute::<
            *const core::ffi::c_void,
            unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
        >(load_library));

        let thread = CreateRemoteThread(process, None, 0, start, Some(remote_mem), 0, None)
            .map_err(|e| {
                let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                format!("CreateRemoteThread failed: {e}")
            })?;

        let _ = WaitForSingleObject(thread, 10_000);
        let _ = CloseHandle(thread);
        let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
        let _ = CloseHandle(process);
        Ok(())
    }
}

/// Resolve module!func in this process. System DLLs are mapped consistently.
unsafe fn get_proc(module: &str, func: &str) -> Option<*const core::ffi::c_void> {
    let m: Vec<u16> = module.encode_utf16().chain(std::iter::once(0)).collect();
    let f: Vec<u8> = func.bytes().chain(std::iter::once(0)).collect();
    let hmod = GetModuleHandleW(PCWSTR(m.as_ptr())).ok()?;
    GetProcAddress(hmod, windows::core::PCSTR(f.as_ptr())).map(|p| p as *const core::ffi::c_void)
}

/// Check whether protection actually took effect by reading target window affinity.
///
/// `GetWindowDisplayAffinity` can read other processes' windows. The result is
/// `(excluded visible top-level windows, total visible top-level windows)`.
/// If total > 0 but excluded == 0, injection may have failed or the visible
/// window may belong to another process such as UWP ApplicationFrameHost.
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
