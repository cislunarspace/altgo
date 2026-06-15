//! 录音模块（跨平台调度）。
//!
//! 平台录音器经 `PlatformRecorder` 别名选择：
//! - Linux：`PulseRecorder`，使用 `parecord`（PulseAudio）。
//! - Windows：`WindowsRecorder`，使用 cpal（WASAPI 后端）。
//!
//! 公共 DSP 工具（格式转换、降混、重采样，平台无关、可在 Linux 上单元测试）见 `dsp`。

mod dsp;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::PulseRecorder;
#[cfg(target_os = "windows")]
pub use windows::WindowsRecorder;

#[cfg(target_os = "linux")]
pub type PlatformRecorder = PulseRecorder;
#[cfg(target_os = "windows")]
pub type PlatformRecorder = WindowsRecorder;

pub trait Recorder: Send {
    fn start_recording(&mut self) -> anyhow::Result<()>;
    fn stop_recording(&self) -> anyhow::Result<Vec<u8>>;
    fn is_recording(&self) -> bool;
}

/// Recorder configuration subset.
#[derive(Debug, Clone)]
pub struct RecorderConfig {
    pub sample_rate: u32,
    pub channels: u32,
}

impl From<&crate::config::Config> for RecorderConfig {
    fn from(cfg: &crate::config::Config) -> Self {
        Self {
            sample_rate: cfg.recorder.sample_rate,
            channels: cfg.recorder.channels,
        }
    }
}
