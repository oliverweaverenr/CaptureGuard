//! Build script: compile protect-dll and embed it into the GUI executable.
//!
//! protect-dll is built in an isolated target directory to avoid workspace lock
//! contention, then copied to OUT_DIR for include_bytes! embedding.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "x86_64-pc-windows-msvc" {
        panic!(
            "CaptureGuard can only be built for x86_64-pc-windows-msvc; current TARGET={target}. \
             Run cargo build --release --bin capture-guard with the Windows x64 MSVC toolchain."
        );
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // protect-dll manifest path; gui and protect-dll are workspace siblings.
    let dll_manifest = manifest_dir
        .parent()
        .unwrap()
        .join("protect-dll")
        .join("Cargo.toml");

    // Re-run this script when protect-dll changes.
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

    // Isolated target directory avoids fighting the outer workspace build lock.
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
        // Strip symbols and avoid PDB references in the final executable.
        .env("RUSTFLAGS", "-C strip=symbols -C debuginfo=0")
        .status()
        .expect("failed to invoke cargo for protect-dll");
    assert!(status.success(), "protect-dll build failed");

    let built = dll_target.join(&target).join("release").join("wsvc.dll");
    let embedded = out_dir.join("payload.bin");
    std::fs::copy(&built, &embedded).unwrap_or_else(|e| {
        panic!("failed to copy {}: {e}", built.display());
    });
}
