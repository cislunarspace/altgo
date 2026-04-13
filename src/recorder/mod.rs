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
