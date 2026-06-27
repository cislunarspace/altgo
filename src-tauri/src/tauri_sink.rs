//! Tauri 管道事件接收器实现。
//!
//! 将管道事件转发为 Tauri 事件、剪贴板操作和浮窗管理。
//!
//! 浮窗物理操作已委托给 `OverlaySink` trait object：本模块只描述「现在应该显示什么阶段」，
//! 不再直接操纵窗口尺寸/位置/显隐。剪贴板和历史记录业务逻辑委托给
//! `voice_pipeline::process_transcription_result`。

use std::sync::Arc;
use tauri::Emitter;

use crate::{
    config,
    history::HistoryStore,
    overlay_window::{OverlaySink, OverlayState},
    pipeline_controller::PipelineStatus,
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
    overlay: Arc<dyn OverlaySink>,
    history_store: HistoryStore,
}

impl TauriPipelineSink {
    pub fn new(
        app: tauri::AppHandle,
        pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
        cfg: Arc<config::Config>,
        history_store: HistoryStore,
        overlay: Arc<dyn OverlaySink>,
    ) -> Self {
        let platform_output = crate::output::PlatformOutput::new();
        Self {
            app,
            pipeline_status,
            prefer_polished: cfg.output.prefer_polished,
            output: Box::new(platform_output),
            overlay,
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

        // 通过 OverlaySink 统一设置悬浮窗状态 —— 一次性 emit + resize + position + show/hide
        let overlay_state = match status {
            "recording" => OverlayState::recording(),
            "processing" => OverlayState::processing(),
            "idle" | "stopped" => OverlayState::hidden(),
            _ => return,
        };
        self.overlay.set_state(overlay_state);
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
        let overlay = self.overlay.clone();
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

                    // 通过 OverlaySink 切换到 done 状态
                    overlay.set_state(OverlayState::done());

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // Test doubles
    // -----------------------------------------------------------------------

    /// Mock `Output` that never touches the real clipboard.
    struct MockOutput;

    impl crate::output::Output for MockOutput {
        fn write_clipboard(&self, _text: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn notify(&self, _title: &str, _body: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn clone_box(&self) -> Box<dyn crate::output::Output> {
            Box::new(MockOutput)
        }
    }

    /// Mock `OverlaySink` that records every `set_state` call.
    struct MockOverlay {
        states: Mutex<Vec<OverlayState>>,
    }

    impl MockOverlay {
        fn new() -> Self {
            Self {
                states: Mutex::new(Vec::new()),
            }
        }

        fn recorded_states(&self) -> Vec<OverlayState> {
            self.states.lock().unwrap().clone()
        }
    }

    impl OverlaySink for MockOverlay {
        fn set_state(&self, state: OverlayState) {
            self.states.lock().unwrap().push(state);
        }
    }

    // -----------------------------------------------------------------------
    // Fixture
    // -----------------------------------------------------------------------

    struct TestFixture {
        sink: TauriPipelineSink,
        status: Arc<std::sync::RwLock<PipelineStatus>>,
        overlay: Arc<MockOverlay>,
        _history_dir: tempfile::TempDir,
        _app: tauri::App<tauri::Wry>,
    }

    fn make_fixture(prefer_polished: bool) -> TestFixture {
        // Build a minimal Tauri app for testing.
        // Use mock_context + noop_assets to avoid embedding the full tauri.conf.json.
        let app = tauri::Builder::<tauri::Wry>::default()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("failed to build test tauri app");

        let status = Arc::new(std::sync::RwLock::new(PipelineStatus::Idle));
        let overlay = Arc::new(MockOverlay::new());
        let history_dir = tempdir().unwrap();
        let history_store = HistoryStore::new(history_dir.path().join("history.json"));

        let mut cfg = config::Config::default();
        cfg.output.prefer_polished = prefer_polished;

        let sink = TauriPipelineSink {
            app: app.handle().clone(),
            pipeline_status: status.clone(),
            prefer_polished,
            output: Box::new(MockOutput),
            overlay: overlay.clone(),
            history_store,
        };

        TestFixture {
            sink,
            status,
            overlay,
            _history_dir: history_dir,
            _app: app,
        }
    }

    // -----------------------------------------------------------------------
    // on_status_change 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_status_change_recording_maps_status_and_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change("recording");

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Recording);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, "recording");
    }

    #[test]
    fn on_status_change_processing_maps_status_and_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change("processing");

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Processing);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, "processing");
    }

    #[test]
    fn on_status_change_done_maps_status_without_overlay_call() {
        let fx = make_fixture(true);
        fx.sink.on_status_change("done");

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Done);
        // "done" does not match recording/processing/idle/stopped => overlay not called
        assert!(fx.overlay.recorded_states().is_empty());
    }

    #[test]
    fn on_status_change_idle_hides_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change("idle");

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, "hidden");
    }

    #[test]
    fn on_status_change_stopped_hides_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change("stopped");

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, "hidden");
    }

    #[test]
    fn on_status_change_unknown_defaults_to_idle_no_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change("garbage");

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
        // Unknown status doesn't match any overlay arm => no overlay call
        assert!(fx.overlay.recorded_states().is_empty());
    }

    // -----------------------------------------------------------------------
    // on_error 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_error_does_not_panic() {
        let fx = make_fixture(true);
        fx.sink.on_error("something went wrong");
        fx.sink.on_error("");
    }

    // -----------------------------------------------------------------------
    // on_transcription_result 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_transcription_result_empty_raw_text_resets_to_idle() {
        let fx = make_fixture(true);

        // Set status to something non-idle first so we can observe the reset.
        *fx.status.write().unwrap() = PipelineStatus::Recording;

        fx.sink.on_transcription_result(&PipelineOutput {
            text: String::new(),
            raw_text: String::new(),
            polish_failed: false,
        });

        // Synchronous early-return: status must be reset to Idle.
        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
    }

    #[test]
    fn on_transcription_result_non_empty_spawns_async() {
        let fx = make_fixture(false);

        fx.sink.on_transcription_result(&PipelineOutput {
            text: "polished".into(),
            raw_text: "raw text".into(),
            polish_failed: false,
        });

        // The async task is spawned; we cannot observe its result synchronously
        // without a tokio runtime, but we can verify we didn't panic and the
        // status wasn't changed synchronously.
    }

    // -----------------------------------------------------------------------
    // on_progress 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_progress_does_not_panic() {
        let fx = make_fixture(true);
        fx.sink.on_progress("transcribe", Some(0.5));
        fx.sink.on_progress("polish", None);
        fx.sink.on_progress("done", Some(1.0));
    }

    // -----------------------------------------------------------------------
    // on_key_listener_backend 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_key_listener_backend_does_not_panic() {
        let fx = make_fixture(true);
        fx.sink.on_key_listener_backend("xinput");
        fx.sink.on_key_listener_backend("evtest");
        fx.sink.on_key_listener_backend("");
    }
}
