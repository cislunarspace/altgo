//! 输出模块（Linux）。
//!
//! 使用 `xclip`/`xsel`/`wl-copy`（剪切板）+ `notify-send`（通知）。

mod linux;

pub use linux::write_clipboard;
