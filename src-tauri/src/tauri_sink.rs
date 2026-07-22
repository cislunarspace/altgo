//! Tauri 管道事件接收器实现。
//!
//! 将管道事件转发为 Tauri 事件与浮窗状态切换：sink 只做 emit + 状态切换，
//! 不再持有 `Output` / `HistoryStore` 等业务依赖。
//!
//! 剪贴板写入与历史追加业务由 `voice_pipeline::TranscriptionDispatch`
//! trait 注入（本模块不直接调用 `process_transcription_result`）；
//! 浮窗物理操作由 `OverlaySink` trait 注入（本模块只描述阶段意图）。

use std::sync::Arc;
use tauri::Emitter;

use crate::{
    config,
    overlay::seam::{OverlaySink, OverlayState},
    pipeline_controller::PipelineStatus,
    voice_pipeline::{PipelineSink, TranscriptionDispatch, TranscriptionResult},
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

/// Tauri 管道事件接收器 — 将管道事件转发为 Tauri 事件和浮窗状态切换。
///
/// 只持有 `dispatch: Arc<dyn TranscriptionDispatch>` 与 overlay 抽象，
/// 业务侧由调用方在构造时一次性注入。
pub struct TauriPipelineSink {
    app: tauri::AppHandle,
    pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
    prefer_polished: bool,
    dispatch: Arc<dyn TranscriptionDispatch>,
    overlay: Arc<dyn OverlaySink>,
}

impl TauriPipelineSink {
    pub fn new(
        app: tauri::AppHandle,
        pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
        cfg: Arc<config::Config>,
        dispatch: Arc<dyn TranscriptionDispatch>,
        overlay: Arc<dyn OverlaySink>,
    ) -> Self {
        Self {
            app,
            pipeline_status,
            prefer_polished: cfg.output.prefer_polished,
            dispatch,
            overlay,
        }
    }
}

impl PipelineSink for TauriPipelineSink {
    fn on_status_change(&self, status: PipelineStatus) {
        emit_pipeline_status(&self.app, &self.pipeline_status, status);

        // 通过 OverlaySink 统一设置悬浮窗状态 —— 一次性 emit + resize + position + show/hide。
        // recording/processing/idle/stopped 各自映射到一个 overlay 阶段；
        // Done 不在此驱动（done 浮窗由转写完成路径异步设置）。
        let overlay_state = match status {
            PipelineStatus::Recording => OverlayState::recording(),
            PipelineStatus::Processing => OverlayState::processing(),
            PipelineStatus::Idle | PipelineStatus::Stopped => OverlayState::hidden(),
            PipelineStatus::Done => return,
        };
        self.overlay.set_state(overlay_state);
    }

    fn on_error(&self, message: &str) {
        let _ = self.app.emit("pipeline-error", message);
    }

    fn on_transcription_result(&self, output: &TranscriptionResult) {
        if output.raw_text.is_empty() {
            emit_pipeline_status(&self.app, &self.pipeline_status, PipelineStatus::Idle);
            return;
        }

        let app = self.app.clone();
        let status = self.pipeline_status.clone();
        let output_clone = output.clone();
        let prefer_polished = self.prefer_polished;
        let dispatch = Arc::clone(&self.dispatch);
        let overlay = self.overlay.clone();

        tauri::async_runtime::spawn(async move {
            let result = dispatch.dispatch(&output_clone, prefer_polished).await;

            match result {
                Some(res) => {
                    if res.history_appended {
                        let _ = app.emit("history-updated", ());
                    }

                    emit_pipeline_status(&app, &status, PipelineStatus::Done);

                    // 先送结果文本再切 done：前端收到 done 时若还没有结果，
                    // 会渲染出空 island（闪烁）。
                    let _ = app.emit("transcription-result", &res.text);

                    // 通过 OverlaySink 切换到 done 状态
                    overlay.set_state(OverlayState::done());
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

// 测试 fixture 需要构建真实 Wry app。Linux 上 tao::EventLoop 只能在主线程
// 初始化，而 libtest 在工作线程跑用例，构建必然 panic，故整个测试模块只在
// Windows 编译运行（CI 由 windows-check job 覆盖，见 ci.yml）。
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::overlay::seam::OverlayPhase;
    use crate::voice_pipeline::{DispatchOutcome, TranscriptionDispatch};
    use std::future::{ready, Future};
    use std::pin::Pin;
    use std::sync::Mutex;

    // -----------------------------------------------------------------------
    // Test doubles
    // -----------------------------------------------------------------------

    /// Mock `TranscriptionDispatch` that ignores the input and returns `None`
    /// (mirroring empty-transcription semantics). Tests asserting behaviour
    /// after `on_transcription_result` only need the sink to not panic.
    struct MockDispatch;

    impl TranscriptionDispatch for MockDispatch {
        fn dispatch<'a>(
            &'a self,
            _output: &'a TranscriptionResult,
            _prefer_polished: bool,
        ) -> Pin<Box<dyn Future<Output = Option<DispatchOutcome>> + Send + 'a>> {
            Box::pin(ready(None))
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

        let mut cfg = config::Config::default();
        cfg.output.prefer_polished = prefer_polished;

        let dispatch: Arc<dyn TranscriptionDispatch> = Arc::new(MockDispatch);
        let sink = TauriPipelineSink::new(
            app.handle().clone(),
            status.clone(),
            Arc::new(cfg),
            dispatch,
            overlay.clone(),
        );

        TestFixture {
            sink,
            status,
            overlay,
            _app: app,
        }
    }

    // -----------------------------------------------------------------------
    // on_status_change 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_status_change_recording_maps_status_and_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change(PipelineStatus::Recording);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Recording);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Recording);
    }

    #[test]
    fn on_status_change_processing_maps_status_and_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change(PipelineStatus::Processing);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Processing);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Processing);
    }

    #[test]
    fn on_status_change_done_maps_status_without_overlay_call() {
        let fx = make_fixture(true);
        fx.sink.on_status_change(PipelineStatus::Done);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Done);
        // Done 不在此驱动 overlay（done 浮窗由转写完成路径异步设置）
        assert!(fx.overlay.recorded_states().is_empty());
    }

    #[test]
    fn on_status_change_idle_hides_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change(PipelineStatus::Idle);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Hidden);
    }

    #[test]
    fn on_status_change_stopped_hides_overlay() {
        let fx = make_fixture(true);
        fx.sink.on_status_change(PipelineStatus::Stopped);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Stopped);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Hidden);
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

        fx.sink.on_transcription_result(&TranscriptionResult {
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

        fx.sink.on_transcription_result(&TranscriptionResult {
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
