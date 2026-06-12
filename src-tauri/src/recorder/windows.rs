//! Windows recorder stub.
//!
//! Windows audio capture integration will live here. For now the pipeline uses
//! the platform alias and fails gracefully at runtime on Windows.

use crate::recorder::Recorder;
use anyhow::Result;

pub struct WindowsRecorder;

impl WindowsRecorder {
    pub fn new(_sample_rate: u32, _channels: u32) -> Self {
        Self
    }
}

impl Recorder for WindowsRecorder {
    fn start_recording(&mut self) -> Result<()> {
        anyhow::bail!("Windows recorder is not implemented yet")
    }

    fn stop_recording(&self) -> Result<Vec<u8>> {
        anyhow::bail!("Windows recorder is not implemented yet")
    }

    fn is_recording(&self) -> bool {
        false
    }
}
