//! CaptureGuard —— 截屏防护工具（GUI）。
//!
//! 选择一个有窗口的进程，点击"开启防护"，把它的窗口设为截屏排除
//! （任何截屏/录屏都拍不到，自己看正常）。注入后可关闭本程序，防护持续生效；
//! 下次打开本程序可对已注入的进程"解除防护"。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod inject;
mod proc;
mod selfprotect;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use proc::ProcEntry;

fn main() -> eframe::Result<()> {
    // 隐藏自检入口：--selftest <pid>，无界面跑 注入→检测→解除，用于验证单文件释放注入链路。
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--selftest" {
        run_selftest(&args[2]);
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 460.0])
            .with_min_inner_size([420.0, 320.0])
            .with_title("CaptureGuard 截屏防护"),
        ..Default::default()
    };
    eframe::run_native(
        "CaptureGuard 截屏防护",
        options,
        Box::new(|cc| {
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}

/// 无界面自检：用生产路径（释放内嵌 DLL）跑完整注入握手。
fn run_selftest(pid_str: &str) {
    let pid: u32 = pid_str.parse().expect("pid 必须是数字");
    let dll = match locate_dll() {
        Ok(p) => p,
        Err(e) => {
            println!("释放内嵌 DLL 失败: {e}");
            return;
        }
    };
    println!("内嵌 DLL 已释放到: {}", dll.display());
    println!("注入前 is_protected = {}", inject::is_protected(pid));
    match inject::inject(pid, &dll) {
        Ok(()) => println!("注入成功"),
        Err(e) => {
            println!("注入失败: {e}");
            return;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
    println!("注入后 is_protected = {}", inject::is_protected(pid));
    let (excluded, total) = inject::protected_window_stats(pid);
    println!("窗口校验：{excluded}/{total} 个可见顶层窗口已设为截屏排除");
    if total > 0 && excluded == 0 {
        println!("⚠ 注入似乎未生效（可能被拦截，或目标窗口属于其它进程，如 UWP 的 ApplicationFrameHost）");
    }
    match inject::request_unprotect(pid) {
        Ok(()) => println!("已请求解除"),
        Err(e) => println!("解除失败: {e}"),
    }
    std::thread::sleep(std::time::Duration::from_millis(1200));
    println!("解除后 is_protected = {}", inject::is_protected(pid));
}

/// 加载系统中文字体，否则中文显示为方块。
fn install_cjk_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // 优先微软雅黑，退化到黑体。
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(bytes));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            break;
        }
    }
    ctx.set_fonts(fonts);
}

struct App {
    procs: Vec<ProcEntry>,
    /// 各进程是否已注入：与 procs 同序。
    protected: Vec<bool>,
    selected: Option<usize>,
    status: String,
    last_refresh: Instant,
    dll_path: Option<PathBuf>,
    dll_err: Option<String>,
    /// 是否把本程序自己的窗口也从截屏排除（默认关）。
    self_protected: bool,
}

impl App {
    fn new() -> Self {
        let (dll_path, dll_err) = match locate_dll() {
            Ok(p) => (Some(p), None),
            Err(e) => (None, Some(e)),
        };
        let mut app = App {
            procs: Vec::new(),
            protected: Vec::new(),
            selected: None,
            status: "选择一个进程，点击开启防护。".into(),
            last_refresh: Instant::now(),
            dll_path,
            dll_err,
            self_protected: false,
        };
        app.refresh();
        app
    }

    /// 重新枚举进程并刷新注入状态。
    fn refresh(&mut self) {
        let prev_pid = self.selected.and_then(|i| self.procs.get(i)).map(|p| p.pid);
        self.procs = proc::list_windowed_processes();
        self.protected = self
            .procs
            .iter()
            .map(|p| inject::is_protected(p.pid))
            .collect();
        // 尽量保持原选中项。
        self.selected = prev_pid.and_then(|pid| self.procs.iter().position(|p| p.pid == pid));
        self.last_refresh = Instant::now();
    }

    fn refresh_protection_only(&mut self) {
        self.protected = self
            .procs
            .iter()
            .map(|p| inject::is_protected(p.pid))
            .collect();
        self.last_refresh = Instant::now();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 每帧确保本程序自己的窗口按当前开关设置截屏排除（开销极小）。
        selfprotect::set_self_protected(self.self_protected);

        // 每 2 秒自动刷新注入状态（捕捉外部变化、目标退出等）。
        if self.last_refresh.elapsed() > Duration::from_secs(2) {
            self.refresh_protection_only();
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("截屏防护");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 刷新列表").clicked() {
                        self.refresh();
                        self.status = "已刷新进程列表。".into();
                    }
                    ui.checkbox(&mut self.self_protected, "隐藏本窗口");
                });
            });
            ui.label(
                egui::RichText::new(
                    "对选中进程开启防护后，其窗口在截屏/录屏中将被排除（你自己看正常）。\
                     注入后可关闭本程序，防护持续生效。",
                )
                .small()
                .weak(),
            );
            ui.add_space(6.0);
        });

        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.add_space(6.0);
            if let Some(err) = &self.dll_err {
                ui.colored_label(egui::Color32::RED, format!("⚠ {err}"));
            }
            ui.horizontal(|ui| {
                let sel = self.selected.and_then(|i| self.procs.get(i).cloned());
                let is_prot = self
                    .selected
                    .and_then(|i| self.protected.get(i).copied())
                    .unwrap_or(false);

                let can_act = sel.is_some() && self.dll_path.is_some();

                if ui
                    .add_enabled(can_act && !is_prot, egui::Button::new("🛡 开启防护"))
                    .clicked()
                {
                    if let Some(p) = &sel {
                        self.do_protect(p.pid, &p.name);
                    }
                }
                if ui
                    .add_enabled(can_act && is_prot, egui::Button::new("✖ 解除防护"))
                    .clicked()
                {
                    if let Some(p) = &sel {
                        self.do_unprotect(p.pid, &p.name);
                    }
                }
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(&self.status).small());
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.procs.is_empty() {
                    ui.label("未发现有窗口的进程。");
                    return;
                }
                let protected = self.protected.clone();
                for (i, p) in self.procs.clone().iter().enumerate() {
                    let is_prot = protected.get(i).copied().unwrap_or(false);
                    let selected = self.selected == Some(i);

                    let label = format!("{}  (PID {})", p.name, p.pid);
                    let resp = ui.selectable_label(selected, build_row(&label, &p.title, is_prot));
                    if resp.clicked() {
                        self.selected = Some(i);
                    }
                }
            });
        });
    }
}

impl App {
    fn do_protect(&mut self, pid: u32, name: &str) {
        let Some(dll) = self.dll_path.clone() else {
            self.status = "防护组件不可用。".into();
            return;
        };
        match inject::inject(pid, &dll) {
            Ok(()) => {
                // 注入后回读校验：确认窗口确实被设为截屏排除。
                std::thread::sleep(Duration::from_millis(300));
                let (excluded, total) = inject::protected_window_stats(pid);
                if total > 0 && excluded == 0 {
                    self.status = format!(
                        "已注入 {name}，但未检测到窗口被排除——可能被杀软拦截，\
                         或其窗口属于其它进程（如 UWP/部分浏览器）。录屏可能仍可见。"
                    );
                } else {
                    self.status = format!(
                        "已对 {name} (PID {pid}) 开启防护（{excluded}/{total} 窗口已排除）。"
                    );
                }
                self.refresh_protection_only();
            }
            Err(e) => {
                self.status = format!("开启失败：{e}（可能需管理员权限或被杀软拦截）");
            }
        }
    }

    fn do_unprotect(&mut self, pid: u32, name: &str) {
        match inject::request_unprotect(pid) {
            Ok(()) => {
                self.status = format!("已请求解除 {name} (PID {pid}) 的防护。");
                // DLL 自卸载需要一点时间，稍后刷新状态。
                std::thread::sleep(Duration::from_millis(300));
                self.refresh_protection_only();
            }
            Err(e) => {
                self.status = format!("解除失败：{e}");
            }
        }
    }
}

/// 拼一行展示文本：名称 + 状态标记 + 标题。
fn build_row(label: &str, title: &str, protected: bool) -> String {
    let mark = if protected {
        "🛡 已防护"
    } else {
        "　未防护"
    };
    let title = if title.is_empty() {
        String::new()
    } else {
        format!("  —  {title}")
    };
    format!("{mark}   {label}{title}")
}

/// 把内嵌的 protect_dll.dll 释放到临时目录，返回其路径。
///
/// 注入需要磁盘上的真实 DLL 文件（LoadLibraryW 收路径），无法纯内存注入，
/// 所以单文件分发时运行期把内嵌字节写到 %TEMP%。按内容大小命名，避免重复写。
fn locate_dll() -> Result<PathBuf, String> {
    const DLL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

    let dir = std::env::temp_dir().join(obfstr::obfstr!("winsvc-cache"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;

    // 文件名带字节长度，DLL 更新后文件名变化，自然覆盖旧版本逻辑。
    // 名字也混淆，落地文件不暴露用途。
    let path = dir.join(format!("{}{}.dat", obfstr::obfstr!("ws_"), DLL_BYTES.len()));

    // 已存在且大小一致就复用（可能正被某进程加载锁定，无需重写）。
    let need_write = match std::fs::metadata(&path) {
        Ok(m) => m.len() != DLL_BYTES.len() as u64,
        Err(_) => true,
    };
    if need_write {
        if let Err(e) = std::fs::write(&path, DLL_BYTES) {
            // 写失败大概率是旧文件被占用；若已存在就继续用旧的。
            if !path.exists() {
                return Err(format!("释放防护组件失败: {e}"));
            }
        }
    }
    Ok(path)
}
