# Release Process

Releases are built by GitHub Actions on `windows-latest`.


## Pre-release Checklist

1. Update `workspace.package.version` in `Cargo.toml`.
2. Update README, `docs/`, and change notes if needed.
3. Run the checks on Windows x64:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --bin capture-guard
```

4. Manually verify the core flow:

- The GUI lists processes with visible windows.
- Screenshots/recordings exclude the protected window after protection is
  enabled.
- Protection remains active after closing the GUI.
- Reopening the GUI detects the protected state.
- Disabling protection restores normal capture behavior.


## Create A GitHub Release

Pushing a `v*` tag triggers the Release workflow:

```bash
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

The workflow generates:

```text
capture-guard-v0.2.0-x86_64-windows.exe
```

The `Release` workflow can also be triggered manually from GitHub Actions by
providing a tag.


## Versioning

Use semantic versioning:

- Bug fixes: patch, for example `0.2.1`.
- Compatible features: minor, for example `0.3.0`.
- Breaking changes: major, for example `1.0.0`.


## Post-release Checks

- The Release page has the exe attachment.
- Release notes do not contain local paths or sensitive information.
- README download links reach the Releases page.
- The new version runs on a clean Windows environment.
