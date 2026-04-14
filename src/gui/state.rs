//! GUI state management — drives what the user sees.
//!
//! Uses Mutex for synchronous access from the eframe main thread.
//! A separate async-compatible API exists for IPC-driven updates.

use std::sync::{Arc, Mutex};

#[cfg(target_os = "linux")]
use tao::system_tray::SystemTray;
#[cfg(target_os = "linux")]
use tao::AppHandle;

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
    #[cfg(target_os = "linux")]
    app_handle: Mutex<Option<AppHandle>>,
    #[cfg(target_os = "linux")]
    tray: Mutex<Option<SystemTray>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(GuiState::default()),
            #[cfg(target_os = "linux")]
            app_handle: Mutex::new(None),
            #[cfg(target_os = "linux")]
            tray: Mutex::new(None),
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
        drop(inner);
        self.update_tray();
    }

    /// Set the transcription result.
    pub fn set_transcription(&self, text: String) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.recording = RecordingState::Done;
            inner.transcription = Some(text);
            inner.message = None;
        }
        self.update_tray();
    }

    /// Set a status message.
    #[allow(dead_code)]
    pub fn set_message(&self, msg: impl Into<String>) {
        let mut inner = self.inner.lock().unwrap();
        inner.message = Some(msg.into());
    }

    /// Store the tao AppHandle (call once at startup, Linux only).
    #[cfg(target_os = "linux")]
    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    /// Store the SystemTray (call once at startup, Linux only).
    #[cfg(target_os = "linux")]
    pub fn set_tray(&self, tray: SystemTray) {
        *self.tray.lock().unwrap() = Some(tray);
    }

    /// Update the system tray tooltip to match current recording state (Linux only).
    #[cfg(target_os = "linux")]
    fn update_tray(&self) {
        let state = self.get();
        let tooltip = match state.recording {
            RecordingState::Idle => "altgo — 按住 Alt 说话",
            RecordingState::Recording => "altgo — 正在录音...",
            RecordingState::Processing => "altgo — 处理中...",
            RecordingState::Done => "altgo — 转写完成",
        };

        if let Some(tray) = self.tray.lock().unwrap().as_ref() {
            tray.set_tooltip(Some(tooltip));
        }
        tracing::debug!(tooltip, "tray tooltip updated");
    }

    #[cfg(not(target_os = "linux"))]
    fn update_tray(&self) {
        // No-op on non-Linux platforms.
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
