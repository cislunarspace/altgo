//! GUI state management — drives what the user sees.
//!
//! Uses Mutex for synchronous access from the eframe main thread.
//! A separate async-compatible API exists for IPC-driven updates.

use std::sync::{Arc, Mutex};

/// Current recording state shown in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    /// Idle, waiting for user to hold Alt.
    #[default]
    Idle,
    /// User is holding Alt — actively recording.
    Recording,
    /// Alt released, waiting for transcription.
    Processing,
    /// Transcription complete, result ready.
    Done,
}

/// Immutable state snapshot for the GUI to render.
#[derive(Debug, Clone, Default)]
pub struct GuiState {
    pub recording: RecordingState,
    pub transcription: Option<String>,
    pub message: Option<String>,
}

/// Shared mutable state — lock-free reads for the GUI thread.
pub struct SharedState {
    inner: Mutex<GuiState>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(GuiState::default()),
        }
    }

    /// Get a snapshot of the current state (synchronous, for GUI thread).
    pub fn get(&self) -> GuiState {
        self.inner.lock().unwrap().clone()
    }

    /// Set recording state (callable from any thread).
    pub fn set_recording(&self, state: RecordingState) {
        let mut inner = self.inner.lock().unwrap();
        inner.recording = state;
        if state != RecordingState::Done {
            inner.transcription = None;
        }
    }

    /// Set the transcription result.
    pub fn set_transcription(&self, text: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.recording = RecordingState::Done;
        inner.transcription = Some(text);
        inner.message = None;
    }

    /// Set a status message.
    #[allow(dead_code)]
    pub fn set_message(&self, msg: impl Into<String>) {
        let mut inner = self.inner.lock().unwrap();
        inner.message = Some(msg.into());
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global state instance — initialized once when the GUI starts.
pub fn global_state() -> Arc<SharedState> {
    static STATE: once_cell::sync::Lazy<Arc<SharedState>> =
        once_cell::sync::Lazy::new(|| Arc::new(SharedState::new()));
    STATE.clone()
}
