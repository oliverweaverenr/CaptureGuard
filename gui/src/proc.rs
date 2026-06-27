//! 枚举"拥有可见顶层窗口"的进程，供 GUI 列表展示。
//!
//! 做法：EnumWindows 遍历所有顶层窗口，过滤出可见、有标题、非工具窗口的，
//! 用 GetWindowThreadProcessId 归到 PID，再按 PID 去重、补进程名。

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

/// 一个可注入的进程条目。
#[derive(Clone)]
pub struct ProcEntry {
    pub pid: u32,
    pub name: String,
    /// 代表性窗口标题（取第一个找到的主窗口）。
    pub title: String,
}

/// 返回所有"拥有可见主窗口"的进程，按进程名排序。
pub fn list_windowed_processes() -> Vec<ProcEntry> {
    // 先收集 pid -> 窗口标题。
    let mut map: BTreeMap<u32, String> = BTreeMap::new();
    let map_ptr = &mut map as *mut BTreeMap<u32, String>;
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(map_ptr as isize));
    }

    // 补进程名。
    let names = process_names();
    let mut out: Vec<ProcEntry> = map
        .into_iter()
        .map(|(pid, title)| ProcEntry {
            pid,
            name: names.get(&pid).cloned().unwrap_or_else(|| "<未知>".into()),
            title,
        })
        .collect();
    out.sort_by_key(|entry| entry.name.to_lowercase());
    out
}

/// EnumWindows 回调：筛选主窗口并记录 pid -> 标题。
unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let map = &mut *(lparam.0 as *mut BTreeMap<u32, String>);

    // 只要可见窗口。
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    // 跳过工具窗口（多为悬浮/辅助，不是主界面）。
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TOOLWINDOW.0 != 0 {
        return BOOL(1);
    }
    // 只要顶层窗口（无 owner），过滤对话框/弹窗。
    if !GetWindow(hwnd, GW_OWNER).unwrap_or_default().is_invalid() {
        return BOOL(1);
    }
    // 必须有标题。
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

    // 每个 pid 只记第一个窗口标题。
    map.entry(pid).or_insert(title);
    BOOL(1)
}

/// 快照所有进程，返回 pid -> 进程名。
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
