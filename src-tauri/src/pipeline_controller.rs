//! Pipeline lifecycle management and status tracking.

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::PipelineHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStatus {
    #[default]
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
#[derive(Default)]
pub struct PipelineController {
    handle: Mutex<Option<PipelineHandle>>,
    status: Arc<std::sync::RwLock<PipelineStatus>>,
}

impl PipelineController {
    pub fn new() -> Self {
        Self::default()
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
        let handle = {
            let mut guard = self.handle.lock().await;
            (*guard).take()
        };
        if let Some(h) = handle {
            let _ = h.stop_tx.send(());
            // 等待旧 pipeline 线程完全退出，释放 OS 资源（子进程、设备节点、hook）。
            // 不阻塞 tokio 运行时——把阻塞的 join 移到专用线程池。
            let _ = tokio::task::spawn_blocking(move || {
                let _ = h.thread_handle.join();
            })
            .await;
        }
        if let Ok(mut s) = self.status.write() {
            *s = PipelineStatus::Idle;
        }
    }

    /// Blocking variant for use in the ExitRequested handler.
    /// 等待旧 pipeline 线程完全退出后再返回。
    pub fn stop_blocking(&self) {
        let handle = {
            let mut guard = self.handle.blocking_lock();
            (*guard).take()
        };
        if let Some(h) = handle {
            let _ = h.stop_tx.send(());
            let _ = h.thread_handle.join();
        }
        if let Ok(mut s) = self.status.write() {
            *s = PipelineStatus::Idle;
        }
    }
}
