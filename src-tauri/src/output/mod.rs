//! 输出模块（跨平台调度）。
//!
//! 平台输出经 `cfg` 选择：
//! - Linux：`xclip`/`xsel`/`wl-copy`（剪切板）。
//! - Windows：arboard（剪切板），结果展示由 Tauri overlay 处理。
//!
//! `Output` trait 将剪切板抽象为可替换 seam，使业务层不依赖平台细节。
//! `cmd.rs` 中的 `copy_text` 命令直接调用平台函数，不经过 trait。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::write_clipboard;
#[cfg(target_os = "windows")]
pub use windows::write_clipboard;

/// 平台 output adapter 类型别名。
#[cfg(target_os = "linux")]
pub type PlatformOutput = linux::LinuxOutput;
#[cfg(target_os = "windows")]
pub type PlatformOutput = windows::WindowsOutput;

/// 剪切板写入的抽象接口。
///
/// 由 `voice_pipeline::process_transcription_result` 持有，使输出行为可在测试中替换。
pub trait Output: Send + Sync {
    /// 将文本写入系统剪切板。
    fn write_clipboard(&self, text: &str) -> anyhow::Result<()>;

    /// 支持 clone 为 trait object（用于 `PipelineEventHandler::Clone`）。
    fn clone_box(&self) -> Box<dyn Output>;
}
