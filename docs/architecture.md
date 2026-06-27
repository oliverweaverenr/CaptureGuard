# Architecture

CaptureGuard is composed of two Rust crates:

```text
capture-guard workspace
├── gui
│   ├── main.rs          # GUI state, interaction, and self-test entry
│   ├── proc.rs          # process enumeration for visible main windows
│   ├── inject.rs        # DLL injection, status checks, and unprotect requests
│   ├── selfprotect.rs   # optional protection for CaptureGuard's own window
│   └── build.rs         # builds and embeds protect-dll
└── protect-dll
    └── lib.rs           # target-process window protection and self-unload logic
```


## Core Flow

```text
[GUI process]
  ├── Enumerate processes with visible windows
  ├── Extract embedded DLL to the temp directory
  ├── OpenProcess + VirtualAllocEx + WriteProcessMemory
  └── CreateRemoteThread(LoadLibraryW)
             │
             ▼
[Target process]
  ├── Load protect-dll
  ├── Create Local\CaptureGuardUnload_<PID> event
  ├── Refresh window affinity every 250 ms
  └── Restore windows and call FreeLibraryAndExitThread on unload event
```


## Process Enumeration

`gui/src/proc.rs` uses `EnumWindows` to enumerate top-level windows and filters
out:

- Invisible windows.
- Tool windows.
- Owned popups.
- Untitled windows.

It then uses `CreateToolhelp32Snapshot` to resolve process names and sorts the
list for display.


## Injection And Status Checks

`gui/src/inject.rs` uses classic `LoadLibraryW` remote-thread injection:

1. Open the target process.
2. Allocate memory in the target process.
3. Write the DLL path into target memory.
4. Create a remote thread that calls `LoadLibraryW`.
5. Free the remote memory and close handles.

Status detection does not scan module lists. Instead, the GUI tries to open the
named event created by the target DLL:

```text
Local\CaptureGuardUnload_<PID>
```

If the event can be opened, the DLL is running inside the target process.


## Window Protection

`protect-dll/src/lib.rs` calls this API from inside the target process:

```text
SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
```

The DLL applies the setting to visible top-level windows owned by the target
process and recursively to their child windows. This covers common UI stacks
where actual content is rendered in child windows, such as Chromium/CEF, Qt, and
some games.

On systems that do not support `WDA_EXCLUDEFROMCAPTURE`, the DLL falls back to
`WDA_MONITOR`.


## Persistence And Unload

After loading, the DLL starts a worker thread. The thread refreshes window
protection every 250 ms to cover newly created windows, popups, and delayed
rendering surfaces.

Closing the GUI does not affect the worker thread inside the target process. To
disable protection, the GUI signals the named event. The DLL then:

1. Restores target windows to `WDA_NONE`.
2. Closes the named event handle.
3. Calls `FreeLibraryAndExitThread` to unload itself.


## Build Model

During release builds, `gui/build.rs` builds `protect-dll` separately and copies
the DLL output to `payload.bin`, which is embedded into the GUI executable.

At runtime, the GUI extracts the embedded bytes to the user temp directory.
`LoadLibraryW` requires a real file path, so this project does not attempt
memory-only loading.
