#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

// Re-export platform-specific functions with a uniform API.
#[cfg(target_os = "linux")]
pub use linux::{notify_processing, notify_result, write_clipboard};

#[cfg(target_os = "macos")]
pub use macos::{notify_processing, notify_result, write_clipboard};

#[cfg(target_os = "windows")]
pub use windows::{notify_processing, notify_result, write_clipboard};

/// Truncate text to max bytes, respecting UTF-8 boundaries.
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
