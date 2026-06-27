# Contributing

Thanks for your interest in CaptureGuard. This project touches Windows native
APIs, DLL injection, and capture protection, so contributions need to account
for code quality, user safety, and responsible-use boundaries.


## Development Environment

Recommended environment:

- Windows 10/11 x64
- Rust stable
- `x86_64-pc-windows-msvc` target
- Visual Studio Build Tools or Visual Studio with MSVC

Install the Rust target:

```powershell
rustup target add x86_64-pc-windows-msvc
```


## Local Checks

Before submitting changes, run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --bin capture-guard
```

If you change injection, unload, or window-enumeration behavior, include manual
test notes:

- Target process name and bitness.
- Whether CaptureGuard was run as administrator.
- Whether screenshots/recordings exclude the protected window.
- Whether protection remains active after closing the GUI.
- Whether reopening the GUI detects and disables protection correctly.


## Pull Requests

- Keep changes focused. One PR should solve one clear problem.
- Do not commit build artifacts, logs, temporary files, or local IDE settings.
- Update README or `docs/` for user-visible behavior changes.
- Update `docs/release.md` for release-flow changes.
- For high-risk changes, describe impact and rollback steps in the PR.


## Out Of Scope

The project will not accept contributions that add or document:

- Bypassing security software, EDR, DRM, exam monitoring, or workplace
  monitoring.
- Stealth persistence, privilege escalation, anti-forensics, or audit evasion.
- Default injection into sensitive system processes or protected third-party
  products.
- Abuse-oriented tutorials, scripts, or configuration.

If you are unsure whether a change fits the project, open an issue first and
describe the intended use case.
