//! 输出模块（跨平台调度）。
//!
//! 平台输出经 `cfg` 选择：
//! - Linux：`xclip`/`xsel`/`wl-copy`（剪切板）。
//! - Windows：arboard（剪切板）。
//!
//! 结果展示由 Tauri overlay 负责（跨平台），本模块仅处理剪切板写入。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::write_clipboard;
#[cfg(target_os = "windows")]
pub use windows::write_clipboard;
