# 发布流程

本项目通过 GitHub Actions 在 `windows-latest` 上构建 release 产物。


## 发布前检查

1. 确认 `Cargo.toml` 中的 `workspace.package.version` 已更新。
2. 确认 README、`docs/` 和变更说明已同步。
3. 在 Windows x64 环境运行：

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --bin capture-guard
```

4. 手动验证核心流程：

- GUI 能列出有可见窗口的进程。
- 开启防护后截图/录屏排除目标窗口。
- 关闭 GUI 后防护继续生效。
- 重新打开 GUI 后能识别已防护状态。
- 解除防护后目标窗口恢复可截屏。


## 创建 GitHub Release

推送 `v*` tag 会触发 Release workflow：

```bash
git tag v0.2.0
git push origin v0.2.0
```

工作流会生成：

```text
capture-guard-v0.2.0-x86_64-windows.exe
```

也可以在 GitHub Actions 页面手动触发 `Release` workflow，并填写目标 tag。


## 版本号规则

建议使用语义化版本：

- 修复 bug：patch，例如 `0.2.1`。
- 增加兼容功能：minor，例如 `0.3.0`。
- 破坏性变更：major，例如 `1.0.0`。


## 发布后检查

- Release 页面存在 exe 附件。
- Release note 没有包含不应公开的本地路径或敏感信息。
- README 中的下载链接能跳转到 Releases。
- 新版本可以在干净 Windows 环境中直接运行。
