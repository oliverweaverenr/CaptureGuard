//! 构建脚本：编译 protect-dll 并把产物嵌入 gui 可执行文件，实现单文件分发。
//!
//! 用独立 target 目录编译 protect-dll，避开与外层 workspace 构建的锁冲突，
//! 再把 protect_dll.dll 拷到 OUT_DIR，main.rs 用 include_bytes! 嵌入。

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "x86_64-pc-windows-msvc" {
        panic!(
            "CaptureGuard 只能构建为 x86_64-pc-windows-msvc；当前 TARGET={target}。\
             请在 Windows x64 MSVC 工具链下运行 cargo build --release --bin capture-guard。"
        );
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // protect-dll 的 manifest 路径（gui 与 protect-dll 同为 workspace 成员）。
    let dll_manifest = manifest_dir
        .parent()
        .unwrap()
        .join("protect-dll")
        .join("Cargo.toml");

    // 改动 protect-dll 源码时重跑本脚本。
    println!(
        "cargo:rerun-if-changed={}",
        dll_manifest
            .parent()
            .unwrap()
            .join("src")
            .join("lib.rs")
            .display()
    );
    println!("cargo:rerun-if-changed={}", dll_manifest.display());

    // 独立 target 目录，避免与外层构建争用同一锁。
    let dll_target = out_dir.join("protect-dll-build");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .args([
            "build",
            "--release",
            "--manifest-path",
            dll_manifest.to_str().unwrap(),
            "--target",
            &target,
            "--target-dir",
            dll_target.to_str().unwrap(),
        ])
        // 去符号 + 不生成 PDB 引用，避免 protect_dll.pdb / 符号名嵌进最终 exe。
        .env("RUSTFLAGS", "-C strip=symbols -C debuginfo=0")
        .status()
        .expect("调用 cargo 编译 protect-dll 失败");
    assert!(status.success(), "protect-dll 编译失败");

    let built = dll_target.join(&target).join("release").join("wsvc.dll");
    let embedded = out_dir.join("payload.bin");
    std::fs::copy(&built, &embedded).unwrap_or_else(|e| {
        panic!("拷贝 {} 失败: {e}", built.display());
    });
}
