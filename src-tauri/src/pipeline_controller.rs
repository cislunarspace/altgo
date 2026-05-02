//! Pipeline lifecycle management and status tracking.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::PipelineHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStatus {
    Idle,
    Recording,
    Processing,
    Done,
}

impl PipelineStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Processing => "processing",
            Self::Done => "done",
        }
    }
}

/// Manages pipeline lifecycle. Owns the run handle and shared status arc.
///
/// Callers spawn the pipeline thread externally and inject the handle via
/// `start_with` / `start_with_blocking`, keeping this module free of Tauri
/// and sink dependencies.
pub struct PipelineController {
    handle: Mutex<Option<PipelineHandle>>,
    status: Arc<std::sync::RwLock<PipelineStatus>>,
}

impl PipelineController {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
            status: Arc::new(std::sync::RwLock::new(PipelineStatus::Idle)),
        }
    }

    /// Clone of the shared status arc — passed to the sink at spawn time.
    pub fn status_arc(&self) -> Arc<std::sync::RwLock<PipelineStatus>> {
        self.status.clone()
    }

    pub fn current_status(&self) -> PipelineStatus {
        *self.status.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Start the pipeline using the provided spawn closure.
    /// Returns an error if a pipeline is already running.
    pub async fn start_with<F: FnOnce() -> PipelineHandle>(&self, spawn: F) -> Result<(), String> {
        let mut guard = self.handle.lock().await;
        if guard.is_some() {
            return Err("pipeline already running".into());
        }
        *guard = Some(spawn());
        Ok(())
    }

    /// Blocking variant for use in synchronous setup contexts.
    pub fn start_with_blocking<F: FnOnce() -> PipelineHandle>(
        &self,
        spawn: F,
    ) -> Result<(), String> {
        let mut guard = self.handle.blocking_lock();
        if guard.is_some() {
            return Err("pipeline already running".into());
        }
        *guard = Some(spawn());
        Ok(())
    }

    pub async fn stop(&self) {
        let mut guard = self.handle.lock().await;
        if let Some(h) = guard.take() {
            let _ = h.stop_tx.send(());
        }
        if let Ok(mut s) = self.status.write() {
            *s = PipelineStatus::Idle;
        }
    }

    /// Stop, wait for wind-down, then start with a new spawn closure.
    pub async fn restart_with<F: FnOnce() -> PipelineHandle>(
        &self,
        spawn: F,
    ) -> Result<(), String> {
        self.stop().await;
        tokio::time::sleep(Duration::from_millis(320)).await;
        self.start_with(spawn).await
    }

    /// Blocking variant for use in the ExitRequested handler.
    pub fn stop_blocking(&self) {
        let mut guard = self.handle.blocking_lock();
        if let Some(h) = guard.take() {
            let _ = h.stop_tx.send(());
        }
        if let Ok(mut s) = self.status.write() {
            *s = PipelineStatus::Idle;
        }
    }
}
