//! 输出模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的函数接口：
//! `write_clipboard`、`notify_processing`、`notify_result`。
//!
//! - Linux：`xclip`/`xsel`/`wl-copy`（剪切板）+ `notify-send`（通知）
//! - macOS：`pbcopy`（剪切板）+ `osascript`（通知）
//! - Windows：`clip.exe`（UTF-16LE）+ PowerShell/WPF 浮动通知
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
pub use linux::{notify, notify_processing, notify_result, write_clipboard};

#[cfg(target_os = "macos")]
pub use macos::{notify, notify_processing, notify_result, write_clipboard};

#[cfg(target_os = "windows")]
pub use windows::{notify, notify_processing, notify_result, write_clipboard};

/// 截断文本到指定字节数，尊重 UTF-8 字符边界。
///
/// 超过 `max_len` 的文本会被截断并附加 `...`。
pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    // Find a safe truncation point (don't cut multi-byte chars).
    let mut end = max_len;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}
