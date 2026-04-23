//! 输出模块（Linux）。
//!
//! 使用 `xclip`/`xsel`/`wl-copy`（剪切板）+ `notify-send`（通知）。
//! 还提供 `truncate_text` 工具函数，用于安全地截断 UTF-8 文本。

mod linux;

pub use linux::{
    close_recording_window, notify, notify_processing, notify_result, output_text,
    show_recording_window, write_clipboard,
};

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
