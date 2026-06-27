# 贡献指南

感谢你愿意参与 CaptureGuard。这个项目涉及 Windows 原生 API、DLL 注入和窗口防护，
贡献时请同时关注代码质量、用户安全和负责任使用边界。


## 开发环境

推荐环境：

- Windows 10/11 x64
- Rust stable
- `x86_64-pc-windows-msvc` target
- Visual Studio Build Tools 或 Visual Studio 的 MSVC 工具链

安装 Rust target：

```powershell
rustup target add x86_64-pc-windows-msvc
```


## 本地验证

提交前请至少运行：

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --bin capture-guard
```

如果修改了注入、卸载或窗口枚举逻辑，请补充手动验证结果：

- 目标进程名称和位数。
- 是否以管理员权限运行。
- 开启防护后截图/录屏是否排除窗口。
- 关闭 GUI 后防护是否仍持续。
- 重新打开 GUI 后是否能识别并解除防护。


## Pull Request 要求

- 保持改动聚焦，一个 PR 解决一个明确问题。
- 不提交构建产物、日志、临时文件或本地 IDE 配置。
- 涉及用户可见行为时，同步更新 README 或 `docs/`。
- 涉及发布流程时，同步更新 `docs/release.md`。
- 对风险较高的改动，在 PR 描述中说明影响范围和回滚方式。


## 不接受的贡献类型

项目不会接受以下能力或文档：

- 绕过安全软件、EDR、DRM、监考系统或企业监控。
- 隐蔽持久化、提权、反取证或规避审计。
- 默认注入敏感系统进程或第三方受保护进程。
- 面向滥用场景的教程、脚本或配置。

如果你不确定某个改动是否合适，请先开 issue 说明背景和预期用途。
