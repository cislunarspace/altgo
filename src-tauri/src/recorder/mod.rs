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

pub use crate::error::RecorderError;

pub trait Recorder: Send {
    fn start_recording(&mut self) -> Result<(), RecorderError>;
    fn stop_recording(&self) -> Result<Vec<u8>, RecorderError>;
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

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::PulseRecorder;
    use crate::error::RecorderError;

    /// Verify the typed error variants surface at the trait boundary —
    /// the `from anyhow::Error` escape hatch was removed (issue #45), so
    /// `start_recording` / `stop_recording` must hand back a
    /// `RecorderError` directly. Anything wrapping `anyhow` here would
    /// be a regression.
    #[test]
    fn pulse_recorder_returns_typed_recorder_error_on_empty_stop() {
        let rec = PulseRecorder::new(16000, 1);
        // No recording thread started; stop_recording joins nothing,
        // sees an empty buffer, and returns EmptyRecording.
        let err = rec.stop_recording().unwrap_err();
        assert!(matches!(err, RecorderError::EmptyRecording));
    }
}
