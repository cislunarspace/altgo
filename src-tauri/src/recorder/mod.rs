//! 录音模块。
//!
//! Linux：`PulseRecorder`（`parecord`）。Windows：`WindowsRecorder`（cpal / WASAPI）。

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

/// SenseVoice 的固定输入采样率（Hz）。
pub const SAMPLE_RATE: u32 = 16_000;

pub trait Recorder: Send {
    fn start_recording(&mut self) -> Result<(), RecorderError>;
    fn stop_recording(&self) -> Result<Vec<u8>, RecorderError>;
    fn is_recording(&self) -> bool;
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{PulseRecorder, SAMPLE_RATE};
    use crate::error::RecorderError;
    use crate::recorder::Recorder;

    /// Verify the typed error variants surface at the trait boundary —
    /// the `from anyhow::Error` escape hatch was removed (issue #45), so
    /// `start_recording` / `stop_recording` must hand back a
    /// `RecorderError` directly. Anything wrapping `anyhow` here would
    /// be a regression.
    #[test]
    fn pulse_recorder_returns_typed_recorder_error_on_empty_stop() {
        let rec = PulseRecorder::new(SAMPLE_RATE);
        // No recording thread started; stop_recording joins nothing,
        // sees an empty buffer, and returns EmptyRecording.
        let err = rec.stop_recording().unwrap_err();
        assert!(matches!(err, RecorderError::EmptyRecording));
    }
}
