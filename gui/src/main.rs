//! CaptureGuard GUI.
//!
//! Select a windowed process, enable capture protection, and keep protection
//! active after the GUI closes. Reopen the GUI later to disable protection.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod i18n;
mod inject;
mod proc;
mod selfprotect;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use i18n::Language;
use proc::ProcEntry;

fn main() -> eframe::Result<()> {
    let lang = Language::detect();

    // Hidden self-test entry for the single-file extraction/injection path.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--selftest" {
        run_selftest(&args[2], lang);
        return Ok(());
    }

    let title = lang.window_title();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 460.0])
            .with_min_inner_size([420.0, 320.0])
            .with_title(title),
        ..Default::default()
    };
    eframe::run_native(
        title,
        options,
        Box::new(|cc| {
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(App::new(lang)))
        }),
    )
}

/// Headless self-test for the production DLL extraction and injection path.
fn run_selftest(pid_str: &str, lang: Language) {
    let pid: u32 = pid_str.parse().expect(lang.selftest_invalid_pid());
    let dll = match locate_dll() {
        Ok(p) => p,
        Err(e) => {
            println!("{}", lang.selftest_extract_failed(&e));
            return;
        }
    };
    println!("{}", lang.selftest_extracted_to(&dll.display().to_string()));
    println!("before is_protected = {}", inject::is_protected(pid));
    match inject::inject(pid, &dll) {
        Ok(()) => println!("{}", lang.selftest_inject_success()),
        Err(e) => {
            println!("{}", lang.selftest_inject_failed(&e));
            return;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
    println!("after inject is_protected = {}", inject::is_protected(pid));
    let (excluded, total) = inject::protected_window_stats(pid);
    println!("{}", lang.selftest_window_stats(excluded, total));
    if total > 0 && excluded == 0 {
        println!("⚠ {}", lang.selftest_no_effect());
    }
    match inject::request_unprotect(pid) {
        Ok(()) => println!("{}", lang.selftest_unprotect_requested()),
        Err(e) => println!("{}", lang.selftest_unprotect_failed(&e)),
    }
    std::thread::sleep(std::time::Duration::from_millis(1200));
    println!(
        "after unprotect is_protected = {}",
        inject::is_protected(pid)
    );
}

/// Load a system CJK font so localized text and process titles render correctly.
fn install_cjk_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Prefer Microsoft YaHei, then fall back to common Chinese fonts.
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
    lang: Language,
    procs: Vec<ProcEntry>,
    /// Protection state aligned with `procs`.
    protected: Vec<bool>,
    selected: Option<usize>,
    status: String,
    last_refresh: Instant,
    dll_path: Option<PathBuf>,
    dll_err: Option<String>,
    /// Whether CaptureGuard's own window should be excluded from capture.
    self_protected: bool,
}

impl App {
    fn new(lang: Language) -> Self {
        let (dll_path, dll_err) = match locate_dll() {
            Ok(p) => (Some(p), None),
            Err(e) => (None, Some(e)),
        };
        let mut app = App {
            lang,
            procs: Vec::new(),
            protected: Vec::new(),
            selected: None,
            status: lang.initial_status().into(),
            last_refresh: Instant::now(),
            dll_path,
            dll_err,
            self_protected: false,
        };
        app.refresh();
        app
    }

    /// Re-enumerate processes and refresh protection state.
    fn refresh(&mut self) {
        let prev_pid = self.selected.and_then(|i| self.procs.get(i)).map(|p| p.pid);
        self.procs = proc::list_windowed_processes();
        self.protected = self
            .procs
            .iter()
            .map(|p| inject::is_protected(p.pid))
            .collect();
        // Preserve the previous selection when possible.
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
        // Keep self-protection aligned with the current toggle.
        selfprotect::set_self_protected(self.self_protected);

        // Refresh protection state every 2 seconds to catch external changes.
        if self.last_refresh.elapsed() > Duration::from_secs(2) {
            self.refresh_protection_only();
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(self.lang.heading());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(format!("🔄 {}", self.lang.refresh_button()))
                        .clicked()
                    {
                        self.refresh();
                        self.status = self.lang.refreshed_status().into();
                    }
                    ui.checkbox(&mut self.self_protected, self.lang.hide_self_checkbox());
                });
            });
            ui.label(egui::RichText::new(self.lang.description()).small().weak());
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
                    .add_enabled(
                        can_act && !is_prot,
                        egui::Button::new(format!("🛡 {}", self.lang.protect_button())),
                    )
                    .clicked()
                {
                    if let Some(p) = &sel {
                        self.do_protect(p.pid, &p.name);
                    }
                }
                if ui
                    .add_enabled(
                        can_act && is_prot,
                        egui::Button::new(format!("✖ {}", self.lang.unprotect_button())),
                    )
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
                    ui.label(self.lang.no_processes());
                    return;
                }
                let protected = self.protected.clone();
                for (i, p) in self.procs.clone().iter().enumerate() {
                    let is_prot = protected.get(i).copied().unwrap_or(false);
                    let selected = self.selected == Some(i);

                    let label = format!("{}  (PID {})", p.name, p.pid);
                    let resp = ui.selectable_label(
                        selected,
                        build_row(self.lang, &label, &p.title, is_prot),
                    );
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
            self.status = self.lang.component_unavailable().into();
            return;
        };
        match inject::inject(pid, &dll) {
            Ok(()) => {
                // Read back window state after injection to catch blocked attempts.
                std::thread::sleep(Duration::from_millis(300));
                let (excluded, total) = inject::protected_window_stats(pid);
                if total > 0 && excluded == 0 {
                    self.status = self.lang.injected_but_not_excluded(name);
                } else {
                    self.status = self.lang.protect_success(name, pid, excluded, total);
                }
                self.refresh_protection_only();
            }
            Err(e) => {
                self.status = self.lang.protect_failed(&e);
            }
        }
    }

    fn do_unprotect(&mut self, pid: u32, name: &str) {
        match inject::request_unprotect(pid) {
            Ok(()) => {
                self.status = self.lang.unprotect_requested(name, pid);
                // DLL self-unload takes a moment; refresh shortly after.
                std::thread::sleep(Duration::from_millis(300));
                self.refresh_protection_only();
            }
            Err(e) => {
                self.status = self.lang.unprotect_failed(&e);
            }
        }
    }
}

/// Build a process-list row: protection mark + process label + window title.
fn build_row(lang: Language, label: &str, title: &str, protected: bool) -> String {
    let mark = if protected {
        format!("🛡 {}", lang.protected_mark())
    } else {
        format!("  {}", lang.unprotected_mark())
    };
    let title = if title.is_empty() {
        String::new()
    } else {
        format!("  -  {title}")
    };
    format!("{mark}   {label}{title}")
}

/// Extract the embedded protect DLL to the temp directory and return its path.
///
/// LoadLibraryW requires a real file path, so the single-file executable writes
/// the embedded bytes to %TEMP%. The file name includes the payload size to avoid
/// unnecessary rewrites.
fn locate_dll() -> Result<PathBuf, String> {
    const DLL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

    let dir = std::env::temp_dir().join(obfstr::obfstr!("winsvc-cache"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create temp directory: {e}"))?;

    // The size suffix changes when the DLL changes, naturally selecting a new file.
    let path = dir.join(format!("{}{}.dat", obfstr::obfstr!("ws_"), DLL_BYTES.len()));

    // Reuse an existing file when possible; loaded DLL files may be locked.
    let need_write = match std::fs::metadata(&path) {
        Ok(m) => m.len() != DLL_BYTES.len() as u64,
        Err(_) => true,
    };
    if need_write {
        if let Err(e) = std::fs::write(&path, DLL_BYTES) {
            // If writing fails but the file exists, it is likely locked by a target.
            if !path.exists() {
                return Err(format!("Failed to extract protection component: {e}"));
            }
        }
    }
    Ok(path)
}
