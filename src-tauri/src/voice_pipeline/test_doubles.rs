//! 测试替身（test doubles）— 给 voice_pipeline 子模块共享。
//!
//! 这里只放 `#[cfg(test)]` 用的替身结构体 / 实现，避免每个子模块重复
//! 同一套 fake/mock。

use std::sync::{Arc, Mutex};

use crate::key_listener::KeyListener;
use crate::output::Output;
use crate::pipeline_controller::PipelineStatus;
use crate::recorder::Recorder;

use super::sink::{PipelineOutput, PipelineSink};

// ---------------------------------------------------------------------------
// KeyListener fake
// ---------------------------------------------------------------------------

pub(super) struct FakeListener {
    pub(super) backend: &'static str,
}

impl KeyListener for FakeListener {
    fn start(
        &mut self,
    ) -> anyhow::Result<(
        tokio::sync::mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
        &'static str,
    )> {
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok((rx, self.backend))
    }
}

// ---------------------------------------------------------------------------
// Recorder fake
// ---------------------------------------------------------------------------

pub(super) struct FakeRecorder {
    pub(super) recording: std::sync::atomic::AtomicBool,
    pub(super) audio: Vec<u8>,
}

impl FakeRecorder {
    pub(super) fn new(audio: Vec<u8>) -> Self {
        Self {
            recording: std::sync::atomic::AtomicBool::new(false),
            audio,
        }
    }
}

impl Recorder for FakeRecorder {
    fn start_recording(&mut self) -> Result<(), crate::error::RecorderError> {
        self.recording
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    fn stop_recording(&self) -> Result<Vec<u8>, crate::error::RecorderError> {
        self.recording
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(self.audio.clone())
    }
    fn is_recording(&self) -> bool {
        self.recording.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// PipelineSink mock — 记录状态变化、错误、结果以便断言
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(super) struct MockSink {
    status_changes: Arc<Mutex<Vec<PipelineStatus>>>,
    errors: Arc<Mutex<Vec<String>>>,
    results: Arc<Mutex<Vec<PipelineOutput>>>,
}

impl MockSink {
    pub(super) fn new() -> Self {
        Self {
            status_changes: Arc::new(Mutex::new(Vec::new())),
            errors: Arc::new(Mutex::new(Vec::new())),
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) fn status_changes(&self) -> Vec<PipelineStatus> {
        self.status_changes.lock().unwrap().clone()
    }
}

impl PipelineSink for MockSink {
    fn on_status_change(&self, status: PipelineStatus) {
        self.status_changes.lock().unwrap().push(status);
    }
    fn on_error(&self, message: &str) {
        self.errors.lock().unwrap().push(message.to_string());
    }
    fn on_transcription_result(&self, output: &PipelineOutput) {
        self.results.lock().unwrap().push(output.clone());
    }
    fn on_progress(&self, _: &str, _: Option<f32>) {}
    fn on_key_listener_backend(&self, _: &str) {}
}

// ---------------------------------------------------------------------------
// Output fake — 记录剪贴板写入次数
// ---------------------------------------------------------------------------

pub(super) struct FakeOutput {
    pub(super) clipboard_writes: Arc<Mutex<Vec<String>>>,
}

impl FakeOutput {
    pub(super) fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                clipboard_writes: Arc::clone(&writes),
            },
            writes,
        )
    }
}

impl Output for FakeOutput {
    fn write_clipboard(&self, text: &str) -> anyhow::Result<()> {
        self.clipboard_writes.lock().unwrap().push(text.to_string());
        Ok(())
    }

    fn clone_box(&self) -> Arc<dyn Output> {
        Arc::new(FakeOutput {
            clipboard_writes: Arc::clone(&self.clipboard_writes),
        })
    }
}
