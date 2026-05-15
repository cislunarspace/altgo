//! 录音模块（Linux）。
//!
//! 使用 `parecord`（PulseAudio）录制音频。

mod linux;

pub type PlatformRecorder = linux::PulseRecorder;
