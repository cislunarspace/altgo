# Drop macOS Support — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all macOS-specific code and conditionals, focusing on Windows + Linux only.

**Architecture:** Simple deletion + comment update task. No architectural changes — just remove macOS platform files and update conditionals to reflect Windows + Linux only.

**Tech Stack:** Rust (src-tauri)

---

## File Inventory

### Delete (3 files)
- `src-tauri/src/key_listener/macos.rs`
- `src-tauri/src/recorder/macos.rs`
- `src-tauri/src/output/macos.rs`

### Modify Source (5 files)
- `src-tauri/src/key_listener/mod.rs` — remove macOS mod + type alias
- `src-tauri/src/recorder/mod.rs` — remove macOS mod + type alias
- `src-tauri/src/output/mod.rs` — remove macOS mod + re-exports
- `src-tauri/src/cmd.rs` — update cfg comment on fallback
- `src-tauri/src/resource.rs` — update cfg comments

### Modify Docs (2 files)
- `CONTRIBUTING.md` — remove macOS from platform lists
- `CLAUDE.md` — remove macOS from platform requirements

---

### Task 1: Delete macOS platform files

- [ ] **Step 1: Delete macOS platform files**

Run:
```bash
rm src-tauri/src/key_listener/macos.rs
rm src-tauri/src/recorder/macos.rs
rm src-tauri/src/output/macos.rs
```

- [ ] **Step 2: Verify deletions**

Run:
```bash
git status
```
Expected: 3 deletions shown

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore: delete macOS platform files (key_listener, recorder, output)

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 2: Update `key_listener/mod.rs`

**Current state (lines 1-24):**
```rust
//! 按键监听器模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的类型别名 `PlatformListener`，
//! 实现静态分派，无需 trait 对象。
//!
//! - Linux：`xinput test-xi2`（XInput2 扩展）
//! - macOS：通过内联 Swift 脚本使用 CGEvent tap（需要辅助功能权限）
//! - Windows：PowerShell + `GetAsyncKeyState` 轮询

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformListener = linux::X11Listener;

#[cfg(target_os = "macos")]
pub type PlatformListener = macos::MacOSListener;

#[cfg(target_os = "windows")]
pub type PlatformListener = windows::WindowsListener;
```

- [ ] **Step 1: Update module doc comment and remove macOS conditionals**

Replace the above with:
```rust
//! 按键监听器模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的类型别名 `PlatformListener`，
//! 实现静态分派，无需 trait 对象。
//!
//! - Linux：`xinput test-xi2`（XInput2 扩展）
//! - Windows：PowerShell + `GetAsyncKeyState` 轮询

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformListener = linux::X11Listener;

#[cfg(target_os = "windows")]
pub type PlatformListener = windows::WindowsListener;
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/key_listener/mod.rs
git commit -m "refactor(key_listener): remove macOS support

- Delete macos.rs
- Remove macOS mod and PlatformListener type alias
- Update module doc comment

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 3: Update `recorder/mod.rs`

**Current state (lines 1-27):**
```rust
//! 录音模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的类型别名 `PlatformRecorder`，
//! 实现静态分派。
//!
//! - Linux：`parecord`（PulseAudio）
//! - macOS：`sox`（优先）或 `ffmpeg`（备选）
//! - Windows：`ffmpeg`（优先，使用 dshow）或 `sox`（备选）

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformRecorder = linux::PulseRecorder;

#[cfg(target_os = "macos")]
pub type PlatformRecorder = macos::SoxRecorder;

#[cfg(target_os = "windows")]
pub type PlatformRecorder = windows::WindowsRecorder;

#[cfg(target_os = "windows")]
pub use windows::warmup_device;
```

- [ ] **Step 1: Update module doc comment and remove macOS conditionals**

Replace with:
```rust
//! 录音模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的类型别名 `PlatformRecorder`，
//! 实现静态分派。
//!
//! - Linux：`parecord`（PulseAudio）
//! - Windows：`ffmpeg`（优先，使用 dshow）或 `sox`（备选）

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformRecorder = linux::PulseRecorder;

#[cfg(target_os = "windows")]
pub type PlatformRecorder = windows::WindowsRecorder;

#[cfg(target_os = "windows")]
pub use windows::warmup_device;
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/recorder/mod.rs
git commit -m "refactor(recorder): remove macOS support

- Delete macos.rs
- Remove macOS mod and PlatformRecorder type alias
- Update module doc comment

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 4: Update `output/mod.rs`

**Current state (lines 1-38):**
```rust
//! 输出模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的函数接口：
//! `write_clipboard`、`notify_processing`、`notify_result`。
//!
//! - Linux：`xclip`/`xsel`/`wl-copy`（剪切板）+ `notify-send`（通知）
//! - macOS：`pbcopy`（剪切板）+ `osascript`（通知）
//! - Windows：`Set-Clipboard`（原生 Unicode）+ SendInput 注入光标 + PowerShell/WPF 悬浮窗
//!
//! 还提供 `truncate_text` 工具函数，用于安全地截断 UTF-8 文本。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

// Re-export platform-specific functions with a uniform API.
#[cfg(target_os = "linux")]
pub use linux::{
    close_recording_window, notify, notify_processing, notify_result, output_text,
    show_recording_window, write_clipboard,
};

#[cfg(target_os = "macos")]
pub use macos::{
    close_recording_window, notify, notify_processing, notify_result, output_text,
    show_recording_window, write_clipboard,
};

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::{
    close_recording_window, notify, notify_processing, notify_result, output_text,
    show_recording_window, write_clipboard,
};
```

- [ ] **Step 1: Update module doc comment and remove macOS conditionals**

Replace with:
```rust
//! 输出模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的函数接口：
//! `write_clipboard`、`notify_processing`、`notify_result`。
//!
//! - Linux：`xclip`/`xsel`/`wl-copy`（剪切板）+ `notify-send`（通知）
//! - Windows：`Set-Clipboard`（原生 Unicode）+ SendInput 注入光标 + PowerShell/WPF 悬浮窗
//!
//! 还提供 `truncate_text` 工具函数，用于安全地截断 UTF-8 文本。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

// Re-export platform-specific functions with a uniform API.
#[cfg(target_os = "linux")]
pub use linux::{
    close_recording_window, notify, notify_processing, notify_result, output_text,
    show_recording_window, write_clipboard,
};

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::{
    close_recording_window, notify, notify_processing, notify_result, output_text,
    show_recording_window, write_clipboard,
};
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/output/mod.rs
git commit -m "refactor(output): remove macOS support

- Delete macos.rs
- Remove macOS mod and re-exports
- Update module doc comment

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 5: Update `cmd.rs`

**File:** `src-tauri/src/cmd.rs`

Line 172 currently reads:
```rust
#[cfg(not(target_os = "linux"))]
fn get_focused_monitor_info() -> Option<(f64, f64, f64, f64)> {
```

- [ ] **Step 1: Update comment on the Windows-only fallback**

Change `#[cfg(not(target_os = "linux"))]` to `#[cfg(target_os = "windows")]` since this function is now only needed for Windows (Linux has its own full implementation at line 114).

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/cmd.rs
git commit -m "refactor(cmd): tighten get_focused_monitor_info fallback to Windows only

The non-Linux fallback is now Windows-only since macOS support is removed.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 6: Update `resource.rs`

**File:** `src-tauri/src/resource.rs`

- [ ] **Step 1: Update module doc comment (line 13)**

Change:
```
/// - macOS: 可执行文件同级目录下的 `bin/{name}`
```
To be removed (delete the line).

- [ ] **Step 2: Update cfg comment (line 34)**

Change:
```rust
    #[cfg(not(target_os = "linux"))]
```
To:
```rust
    #[cfg(target_os = "windows")]
```
And update its comment to say Windows instead of non-Linux.

**After changes, the function should be:**
```rust
/// 查找捆绑的二进制文件。
///
/// 搜索逻辑：
/// - Linux: `/usr/lib/altgo/bin/{name}`
/// - Windows: 可执行文件同级目录下的 `bin/{name}`
pub fn bundled_bin(name: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();

    #[cfg(target_os = "linux")]
    {
        // If installed to /usr/bin, look for /usr/lib/altgo/bin/.
        if exe_dir.ends_with("/usr/bin") || exe_dir.ends_with("/usr/local/bin") {
            let candidate = PathBuf::from("/usr/lib/altgo/bin").join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        // Otherwise look relative to the exe.
        let candidate = exe_dir.join("bin").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        let candidate = exe_dir.join("bin").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        None
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/resource.rs
git commit -m "refactor(resource): update bundled_bin for Windows + Linux only

- Remove macOS from doc comment
- Change #[cfg(not(target_os = "linux"))] to #[cfg(target_os = "windows")]
- Update comment to reflect Windows-only

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 7: Update `CONTRIBUTING.md`

**File:** `CONTRIBUTING.md`

- [ ] **Step 1: Update line 54-56** (platform descriptions)

Remove macOS mentions from:
- `key_listener/` description
- `recorder/` description
- `output/` description

- [ ] **Step 2: Update line 91** (macOS system requirement line)

Remove the line `- **macOS**: \`sox\`, \`pbcopy\`, \`osascript\``

- [ ] **Step 3: Update line 58** (cfg example)

Change:
```
- 使用 `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "macos")]` / `#[cfg(target_os = "windows")]`
```
To:
```
- 使用 `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "windows")]`
```

- [ ] **Step 4: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs: remove macOS from CONTRIBUTING.md

- Remove macOS from platform descriptions
- Remove macOS system requirements
- Update cfg example to show only Linux + Windows

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 8: Update `CLAUDE.md`

**File:** `CLAUDE.md`

- [ ] **Step 1: Update "Platform System Requirements" section**

Remove the line `- **macOS**: \`sox\`, \`pbcopy\`, \`osascript\``

The section should read:
```
### Platform System Requirements

- **Linux**: `xinput`, `xmodmap`, `parecord`, `xclip`/`xsel`/`wl-copy`, `notify-send`
- **Windows**: `ffmpeg`, PowerShell
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: remove macOS from CLAUDE.md platform requirements

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run cargo check to verify compilation**

Run:
```bash
cd src-tauri && cargo check
```
Expected: Successful compilation with no errors

- [ ] **Step 2: Verify no macOS references remain in source**

Run:
```bash
grep -r "macos\|darwin\|MacOSListener\|SoxRecorder" src-tauri/src/ --include="*.rs"
```
Expected: No output (all macOS references removed)

- [ ] **Step 3: Commit verification**

```bash
git add -A
git commit -m "chore: verify clean build after macOS removal

- cargo check passes
- no macOS/darwin references in source

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Spec Coverage Check

| Spec Section | Tasks |
|-------------|-------|
| Delete 3 macOS files | Task 1 |
| key_listener/mod.rs | Task 2 |
| recorder/mod.rs | Task 3 |
| output/mod.rs | Task 4 |
| cmd.rs | Task 5 |
| resource.rs | Task 6 |
| CONTRIBUTING.md | Task 7 |
| CLAUDE.md | Task 8 |
| Build verification | Task 9 |
