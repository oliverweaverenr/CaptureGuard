# 架构说明

CaptureGuard 由两个 Rust crate 组成：

```text
capture-guard workspace
├── gui
│   ├── main.rs          # GUI 状态、交互和自检入口
│   ├── proc.rs          # 枚举拥有可见主窗口的进程
│   ├── inject.rs        # DLL 注入、状态检测和解除防护
│   ├── selfprotect.rs   # CaptureGuard 自身窗口防护
│   └── build.rs         # 编译并嵌入 protect-dll
└── protect-dll
    └── lib.rs           # 目标进程内的窗口防护和自卸载逻辑
```


## 核心流程

```text
[GUI 进程]
  ├── 枚举可见窗口进程
  ├── 释放内嵌 DLL 到临时目录
  ├── OpenProcess + VirtualAllocEx + WriteProcessMemory
  └── CreateRemoteThread(LoadLibraryW)
             │
             ▼
[目标进程]
  ├── 加载 protect-dll
  ├── 创建 Local\CaptureGuardUnload_<PID> 事件
  ├── 后台线程每 250ms 刷新窗口 affinity
  └── 收到卸载事件后还原窗口并 FreeLibraryAndExitThread
```


## 进程枚举

`gui/src/proc.rs` 使用 `EnumWindows` 遍历顶层窗口，并过滤：

- 不可见窗口。
- 工具窗口。
- 有 owner 的弹窗。
- 无标题窗口。

随后通过 `CreateToolhelp32Snapshot` 补全进程名，并按进程名排序展示。


## 注入与状态检测

`gui/src/inject.rs` 使用经典 `LoadLibraryW` 远程线程注入：

1. 打开目标进程。
2. 在目标进程分配内存。
3. 写入 DLL 路径。
4. 创建远程线程调用 `LoadLibraryW`。
5. 释放远程内存并关闭句柄。

状态检测不扫描模块列表，而是尝试打开目标 DLL 创建的命名事件：

```text
Local\CaptureGuardUnload_<PID>
```

能打开事件说明 DLL 正在目标进程内运行。


## 窗口防护

`protect-dll/src/lib.rs` 在目标进程内调用：

```text
SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
```

它会同时处理目标进程拥有的可见顶层窗口和子窗口。这样可以覆盖 Chromium/CEF、Qt、
游戏窗口等常见“内容在子窗口绘制”的情况。

在不支持 `WDA_EXCLUDEFROMCAPTURE` 的系统上，会退化为 `WDA_MONITOR`。


## 自维持与解除

DLL 加载后启动一个后台线程。该线程每 250ms 刷新一次窗口防护，以覆盖新建窗口、
弹窗和延迟创建的渲染表面。

GUI 关闭不会影响目标进程内的后台线程。需要解除时，GUI 触发命名事件，DLL 会：

1. 将目标窗口恢复为 `WDA_NONE`。
2. 关闭命名事件句柄。
3. 调用 `FreeLibraryAndExitThread` 自卸载。


## 构建方式

Release 构建时，`gui/build.rs` 会独立编译 `protect-dll`，并将生成的 DLL 复制为
`payload.bin` 嵌入 GUI 可执行文件。

运行时，GUI 将内嵌字节释放到用户临时目录。因为 `LoadLibraryW` 需要真实文件路径，
这里不能直接使用纯内存字节完成加载。
