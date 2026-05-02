//! 录音模块（Linux）。
//!
//! 使用 `parecord`（PulseAudio）录制音频。

mod linux;

pub type PlatformRecorder = linux::PulseRecorder;

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
