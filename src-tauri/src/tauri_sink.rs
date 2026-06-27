//! Tauri 管道事件接收器实现。
//!
//! 将管道事件转发为 Tauri 事件、剪贴板操作和浮窗管理。
//!
//! 浮窗物理操作已委托给 `OverlayManager`：本模块只描述「现在应该显示什么阶段」，
//! 不再直接操纵窗口尺寸/位置/显隐。剪贴板和历史记录业务逻辑委托给
//! `voice_pipeline::process_transcription_result`。

use std::sync::Arc;
use tauri::Emitter;

use crate::{
    config,
    history::HistoryStore,
    overlay_manager::OverlayManager,
    overlay_manager::OverlayState,
    pipeline_controller::PipelineStatus,
    tauri_overlay_window::TauriOverlayWindow,
    voice_pipeline::{process_transcription_result, PipelineOutput, PipelineSink},
};

fn emit_pipeline_status(
    app: &tauri::AppHandle,
    status: &Arc<std::sync::RwLock<PipelineStatus>>,
    value: PipelineStatus,
) {
    let _ = app.emit("pipeline-status", value.as_str());
    if let Ok(mut s) = status.write() {
        *s = value;
    }
}

/// Tauri 管道事件接收器 — 将管道事件转发为 Tauri 事件和系统操作。
pub struct TauriPipelineSink {
    app: tauri::AppHandle,
    pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
    prefer_polished: bool,
    output: Box<dyn crate::output::Output>,
    overlay_manager: OverlayManager<TauriOverlayWindow>,
    history_store: HistoryStore,
}

impl TauriPipelineSink {
    pub fn new(
        app: tauri::AppHandle,
        pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
        cfg: Arc<config::Config>,
        history_store: HistoryStore,
    ) -> Self {
        let platform_output = crate::output::PlatformOutput::new();
        let overlay_manager = OverlayManager::new(TauriOverlayWindow::new(app.clone()));
        Self {
            app,
            pipeline_status,
            prefer_polished: cfg.output.prefer_polished,
            output: Box::new(platform_output),
            overlay_manager,
            history_store,
        }
    }
}

impl PipelineSink for TauriPipelineSink {
    fn on_status_change(&self, status: &str) {
        let ps = match status {
            "recording" => PipelineStatus::Recording,
            "processing" => PipelineStatus::Processing,
            "done" => PipelineStatus::Done,
            _ => PipelineStatus::Idle,
        };
        emit_pipeline_status(&self.app, &self.pipeline_status, ps);

        // 通过 OverlayManager 统一设置悬浮窗状态 —— 一次性 emit + resize + position + show/hide
        let overlay_state = match status {
            "recording" => OverlayState::recording(),
            "processing" => OverlayState::processing(),
            "idle" | "stopped" => OverlayState::hidden(),
            _ => return,
        };
        self.overlay_manager.set_state(overlay_state);
    }

    fn on_error(&self, message: &str) {
        let _ = self.app.emit("pipeline-error", message);
    }

    fn on_transcription_result(&self, output: &PipelineOutput) {
        if output.raw_text.is_empty() {
            emit_pipeline_status(&self.app, &self.pipeline_status, PipelineStatus::Idle);
            return;
        }

        let app = self.app.clone();
        let status = self.pipeline_status.clone();
        let output_clone = output.clone();
        let prefer_polished = self.prefer_polished;
        let output_adapter = self.output.clone_box();
        let overlay_manager = self.overlay_manager.clone();
        let history_store = self.history_store.clone();

        tauri::async_runtime::spawn(async move {
            let result = process_transcription_result(
                &output_clone,
                prefer_polished,
                &*output_adapter,
                &history_store,
            )
            .await;

            match result {
                Some(res) => {
                    if res.history_appended {
                        let _ = app.emit("history-updated", ());
                    }

                    emit_pipeline_status(&app, &status, PipelineStatus::Done);

                    // 通过 OverlayManager 切换到 done 状态
                    overlay_manager.set_state(OverlayState::done());

                    let _ = app.emit("transcription-result", &res.text);
                }
                None => {
                    emit_pipeline_status(&app, &status, PipelineStatus::Idle);
                }
            }
        });
    }

    fn on_progress(&self, phase: &str, fraction: Option<f32>) {
        let _ = self.app.emit(
            "transcription-progress",
            serde_json::json!({ "phase": phase, "fraction": fraction }),
        );
    }

    fn on_key_listener_backend(&self, backend: &str) {
        let _ = self.app.emit("key-listener-backend", backend);
    }
}
