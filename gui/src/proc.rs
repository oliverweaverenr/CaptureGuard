//! Enumerate processes that own visible top-level windows for the GUI list.
//!
//! EnumWindows collects visible, titled, non-tool top-level windows. The window
//! owner PID is resolved with GetWindowThreadProcessId, then process names are
//! filled through a ToolHelp snapshot.

use std::collections::BTreeMap;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

/// A process entry that can be selected for injection.
#[derive(Clone)]
pub struct ProcEntry {
    pub pid: u32,
    pub name: String,
    /// Representative window title, from the first discovered main window.
    pub title: String,
}

/// Return all processes that own visible main windows, sorted by process name.
pub fn list_windowed_processes() -> Vec<ProcEntry> {
    // Collect pid -> window title first.
    let mut map: BTreeMap<u32, String> = BTreeMap::new();
    let map_ptr = &mut map as *mut BTreeMap<u32, String>;
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(map_ptr as isize));
    }

    // Fill process names.
    let names = process_names();
    let mut out: Vec<ProcEntry> = map
        .into_iter()
        .map(|(pid, title)| ProcEntry {
            pid,
            name: names
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| "<unknown>".into()),
            title,
        })
        .collect();
    out.sort_by_key(|entry| entry.name.to_lowercase());
    out
}

/// EnumWindows callback: select main windows and record pid -> title.
unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let map = &mut *(lparam.0 as *mut BTreeMap<u32, String>);

    // Only visible windows.
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    // Skip tool windows, usually floating/auxiliary surfaces.
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TOOLWINDOW.0 != 0 {
        return BOOL(1);
    }
    // Only unowned top-level windows; filter dialogs/popups.
    if !GetWindow(hwnd, GW_OWNER).unwrap_or_default().is_invalid() {
        return BOOL(1);
    }
    // Require a title.
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return BOOL(1);
    }
    let mut buf = vec![0u16; len as usize + 1];
    let got = GetWindowTextW(hwnd, &mut buf);
    if got <= 0 {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(&buf[..got as usize]);

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return BOOL(1);
    }

    // Keep the first title per pid.
    map.entry(pid).or_insert(title);
    BOOL(1)
}

/// Snapshot all processes and return pid -> process name.
fn process_names() -> BTreeMap<u32, String> {
    let mut names = BTreeMap::new();
    unsafe {
        let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(s) => s,
            Err(_) => return names,
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let n = wide_to_string(&entry.szExeFile);
                names.insert(entry.th32ProcessID, n);
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    names
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
