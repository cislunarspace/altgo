//! Tauri 管道事件接收器实现。
//!
//! 将管道事件转发为 Tauri 事件与浮窗状态切换：sink 只做 emit + 状态切换，
//! 不再持有 `Output` / `HistoryStore` 等业务依赖。
//!
//! 剪贴板写入与历史追加业务由 `voice_pipeline::TranscriptionDispatch`
//! trait 注入（本模块不直接调用 `process_transcription_result`）；
//! 浮窗物理操作由 `OverlaySink` trait 注入（本模块只描述阶段意图）；
//! 框架事件发射由 `PipelineEventEmitter` trait 注入，方便测试注入 fake，
//! 无需构造真实 Wry app。

use std::sync::Arc;
use tauri::Emitter;

use crate::{
    config,
    overlay::seam::{OverlaySink, OverlayState},
    pipeline_controller::PipelineStatus,
    voice_pipeline::{PipelineSink, TranscriptionDispatch, TranscriptionResult},
};

/// 管道事件发射 seam。
///
/// `TauriPipelineSink` 把全部 `app.emit(...)` 操作收敛到这里，
/// 生产环境由 `TauriEventEmitter` 转发给 `tauri::AppHandle`，
/// 测试环境注入 `MockEmitter` 即可断言事件内容与顺序。
pub trait PipelineEventEmitter: Send + Sync + 'static {
    fn emit_pipeline_status(&self, status: &str);
    fn emit_pipeline_error(&self, message: &str);
    fn emit_transcription_result(&self, text: &str);
    fn emit_transcription_progress(&self, phase: &str, fraction: Option<f32>);
    fn emit_key_listener_backend(&self, backend: &str);
    fn emit_history_updated(&self);
}

/// 生产实现：把事件转发给 Tauri 前端。
pub struct TauriEventEmitter {
    app: tauri::AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl PipelineEventEmitter for TauriEventEmitter {
    fn emit_pipeline_status(&self, status: &str) {
        let _ = self.app.emit("pipeline-status", status);
    }

    fn emit_pipeline_error(&self, message: &str) {
        let _ = self.app.emit("pipeline-error", message);
    }

    fn emit_transcription_result(&self, text: &str) {
        let _ = self.app.emit("transcription-result", text);
    }

    fn emit_transcription_progress(&self, phase: &str, fraction: Option<f32>) {
        let _ = self.app.emit(
            "transcription-progress",
            serde_json::json!({ "phase": phase, "fraction": fraction }),
        );
    }

    fn emit_key_listener_backend(&self, backend: &str) {
        let _ = self.app.emit("key-listener-backend", backend);
    }

    fn emit_history_updated(&self) {
        let _ = self.app.emit("history-updated", ());
    }
}

fn emit_pipeline_status(
    emitter: &dyn PipelineEventEmitter,
    status: &Arc<std::sync::RwLock<PipelineStatus>>,
    value: PipelineStatus,
) {
    emitter.emit_pipeline_status(value.as_str());
    if let Ok(mut s) = status.write() {
        *s = value;
    }
}

/// Tauri 管道事件接收器 — 将管道事件转发为 Tauri 事件和浮窗状态切换。
///
/// 只持有 `dispatch: Arc<dyn TranscriptionDispatch>` 与 overlay / emitter 抽象，
/// 业务侧由调用方在构造时一次性注入。
pub struct TauriPipelineSink {
    emitter: Arc<dyn PipelineEventEmitter>,
    pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
    prefer_polished: bool,
    dispatch: Arc<dyn TranscriptionDispatch>,
    overlay: Arc<dyn OverlaySink>,
}

impl TauriPipelineSink {
    pub fn new(
        emitter: Arc<dyn PipelineEventEmitter>,
        pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
        cfg: Arc<config::Config>,
        dispatch: Arc<dyn TranscriptionDispatch>,
        overlay: Arc<dyn OverlaySink>,
    ) -> Self {
        Self {
            emitter,
            pipeline_status,
            prefer_polished: cfg.output.prefer_polished,
            dispatch,
            overlay,
        }
    }
}

impl PipelineSink for TauriPipelineSink {
    fn on_status_change(&self, status: PipelineStatus) {
        emit_pipeline_status(&*self.emitter, &self.pipeline_status, status);

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
        self.emitter.emit_pipeline_error(message);
    }

    fn on_transcription_result(&self, output: &TranscriptionResult) {
        if output.raw_text.is_empty() {
            emit_pipeline_status(&*self.emitter, &self.pipeline_status, PipelineStatus::Idle);
            return;
        }

        let emitter = Arc::clone(&self.emitter);
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
                        emitter.emit_history_updated();
                    }

                    emit_pipeline_status(&*emitter, &status, PipelineStatus::Done);

                    // 先送结果文本再切 done：前端收到 done 时若还没有结果，
                    // 会渲染出空 island（闪烁）。
                    emitter.emit_transcription_result(&res.text);

                    // 通过 OverlaySink 切换到 done 状态
                    overlay.set_state(OverlayState::done());
                }
                None => {
                    emit_pipeline_status(&*emitter, &status, PipelineStatus::Idle);
                }
            }
        });
    }

    fn on_progress(&self, phase: &str, fraction: Option<f32>) {
        self.emitter.emit_transcription_progress(phase, fraction);
    }

    fn on_key_listener_backend(&self, backend: &str) {
        self.emitter.emit_key_listener_backend(backend);
    }
}

// 测试通过注入 `PipelineEventEmitter` fake 来验证事件内容与顺序，
// 不再依赖真实 Wry app，因此可在 Linux 的 `cargo test --lib` 下直接运行。
#[cfg(test)]
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

    /// Mock `TranscriptionDispatch`：可预设返回结果。
    struct MockDispatch {
        outcome: Option<DispatchOutcome>,
    }

    impl TranscriptionDispatch for MockDispatch {
        fn dispatch<'a>(
            &'a self,
            _output: &'a TranscriptionResult,
            _prefer_polished: bool,
        ) -> Pin<Box<dyn Future<Output = Option<DispatchOutcome>> + Send + 'a>> {
            Box::pin(ready(self.outcome.clone()))
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

    /// 一次记录的事件调用。
    #[derive(Debug, Clone, PartialEq)]
    enum EmittedEvent {
        PipelineStatus(String),
        PipelineError(String),
        TranscriptionResult(String),
        TranscriptionProgress {
            phase: String,
            fraction: Option<f32>,
        },
        KeyListenerBackend(String),
        HistoryUpdated,
    }

    /// Mock `PipelineEventEmitter`：把每次调用按顺序记录下来。
    struct MockEmitter {
        events: Mutex<Vec<EmittedEvent>>,
    }

    impl MockEmitter {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }

        fn recorded_events(&self) -> Vec<EmittedEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl PipelineEventEmitter for MockEmitter {
        fn emit_pipeline_status(&self, status: &str) {
            self.events
                .lock()
                .unwrap()
                .push(EmittedEvent::PipelineStatus(status.into()));
        }

        fn emit_pipeline_error(&self, message: &str) {
            self.events
                .lock()
                .unwrap()
                .push(EmittedEvent::PipelineError(message.into()));
        }

        fn emit_transcription_result(&self, text: &str) {
            self.events
                .lock()
                .unwrap()
                .push(EmittedEvent::TranscriptionResult(text.into()));
        }

        fn emit_transcription_progress(&self, phase: &str, fraction: Option<f32>) {
            self.events
                .lock()
                .unwrap()
                .push(EmittedEvent::TranscriptionProgress {
                    phase: phase.into(),
                    fraction,
                });
        }

        fn emit_key_listener_backend(&self, backend: &str) {
            self.events
                .lock()
                .unwrap()
                .push(EmittedEvent::KeyListenerBackend(backend.into()));
        }

        fn emit_history_updated(&self) {
            self.events
                .lock()
                .unwrap()
                .push(EmittedEvent::HistoryUpdated);
        }
    }

    // -----------------------------------------------------------------------
    // Fixture
    // -----------------------------------------------------------------------

    struct TestFixture {
        sink: TauriPipelineSink,
        status: Arc<std::sync::RwLock<PipelineStatus>>,
        overlay: Arc<MockOverlay>,
        emitter: Arc<MockEmitter>,
    }

    fn make_fixture(
        prefer_polished: bool,
        dispatch_outcome: Option<DispatchOutcome>,
    ) -> TestFixture {
        let status = Arc::new(std::sync::RwLock::new(PipelineStatus::Idle));
        let overlay = Arc::new(MockOverlay::new());
        let emitter = Arc::new(MockEmitter::new());

        let mut cfg = config::Config::default();
        cfg.output.prefer_polished = prefer_polished;

        let dispatch: Arc<dyn TranscriptionDispatch> = Arc::new(MockDispatch {
            outcome: dispatch_outcome,
        });
        let sink = TauriPipelineSink::new(
            emitter.clone(),
            status.clone(),
            Arc::new(cfg),
            dispatch,
            overlay.clone(),
        );

        TestFixture {
            sink,
            status,
            overlay,
            emitter,
        }
    }

    // -----------------------------------------------------------------------
    // on_status_change 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_status_change_recording_maps_status_and_overlay() {
        let fx = make_fixture(true, None);
        fx.sink.on_status_change(PipelineStatus::Recording);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Recording);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Recording);
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![EmittedEvent::PipelineStatus("recording".into())]
        );
    }

    #[test]
    fn on_status_change_processing_maps_status_and_overlay() {
        let fx = make_fixture(true, None);
        fx.sink.on_status_change(PipelineStatus::Processing);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Processing);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Processing);
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![EmittedEvent::PipelineStatus("processing".into())]
        );
    }

    #[test]
    fn on_status_change_done_maps_status_without_overlay_call() {
        let fx = make_fixture(true, None);
        fx.sink.on_status_change(PipelineStatus::Done);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Done);
        // Done 不在此驱动 overlay（done 浮窗由转写完成路径异步设置）
        assert!(fx.overlay.recorded_states().is_empty());
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![EmittedEvent::PipelineStatus("done".into())]
        );
    }

    #[test]
    fn on_status_change_idle_hides_overlay() {
        let fx = make_fixture(true, None);
        fx.sink.on_status_change(PipelineStatus::Idle);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Hidden);
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![EmittedEvent::PipelineStatus("idle".into())]
        );
    }

    #[test]
    fn on_status_change_stopped_hides_overlay() {
        let fx = make_fixture(true, None);
        fx.sink.on_status_change(PipelineStatus::Stopped);

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Stopped);
        let states = fx.overlay.recorded_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].phase, OverlayPhase::Hidden);
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![EmittedEvent::PipelineStatus("stopped".into())]
        );
    }

    // -----------------------------------------------------------------------
    // on_error 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_error_emits_pipeline_error() {
        let fx = make_fixture(true, None);
        fx.sink.on_error("something went wrong");
        fx.sink.on_error("");

        assert_eq!(
            fx.emitter.recorded_events(),
            vec![
                EmittedEvent::PipelineError("something went wrong".into()),
                EmittedEvent::PipelineError("".into()),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // on_transcription_result 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_transcription_result_empty_raw_text_resets_to_idle() {
        let fx = make_fixture(true, None);

        // Set status to something non-idle first so we can observe the reset.
        *fx.status.write().unwrap() = PipelineStatus::Recording;

        fx.sink.on_transcription_result(&TranscriptionResult {
            text: String::new(),
            raw_text: String::new(),
            polish_failed: false,
        });

        // Synchronous early-return: status must be reset to Idle.
        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Idle);
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![EmittedEvent::PipelineStatus("idle".into())]
        );
    }

    #[tokio::test]
    async fn on_transcription_result_non_empty_dispatches_async_and_emits_in_order() {
        let fx = make_fixture(
            false,
            Some(DispatchOutcome {
                text: "polished text".into(),
                history_appended: true,
            }),
        );

        fx.sink.on_transcription_result(&TranscriptionResult {
            text: "polished".into(),
            raw_text: "raw text".into(),
            polish_failed: false,
        });

        // spawned 任务跑在 tauri::async_runtime 的全局 runtime 上，
        // 与 #[tokio::test] 的 runtime 不同；轮询等待它完成。
        for _ in 0..100 {
            if fx.emitter.recorded_events().len() >= 3 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        assert_eq!(*fx.status.read().unwrap(), PipelineStatus::Done);

        let overlay_states = fx.overlay.recorded_states();
        assert_eq!(overlay_states.len(), 1);
        assert_eq!(overlay_states[0].phase, OverlayPhase::Done);

        // 真实代码顺序：history-updated → pipeline-status(Done) → transcription-result(text)
        // 先送文本再切 done，前端收到 done 时已有结果，避免空 island 闪烁。
        assert_eq!(
            fx.emitter.recorded_events(),
            vec![
                EmittedEvent::HistoryUpdated,
                EmittedEvent::PipelineStatus("done".into()),
                EmittedEvent::TranscriptionResult("polished text".into()),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // on_progress 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_progress_emits_progress() {
        let fx = make_fixture(true, None);
        fx.sink.on_progress("transcribe", Some(0.5));
        fx.sink.on_progress("polish", None);
        fx.sink.on_progress("done", Some(1.0));

        assert_eq!(
            fx.emitter.recorded_events(),
            vec![
                EmittedEvent::TranscriptionProgress {
                    phase: "transcribe".into(),
                    fraction: Some(0.5),
                },
                EmittedEvent::TranscriptionProgress {
                    phase: "polish".into(),
                    fraction: None,
                },
                EmittedEvent::TranscriptionProgress {
                    phase: "done".into(),
                    fraction: Some(1.0),
                },
            ]
        );
    }

    // -----------------------------------------------------------------------
    // on_key_listener_backend 测试
    // -----------------------------------------------------------------------

    #[test]
    fn on_key_listener_backend_emits_backend() {
        let fx = make_fixture(true, None);
        fx.sink.on_key_listener_backend("xinput");
        fx.sink.on_key_listener_backend("evtest");
        fx.sink.on_key_listener_backend("");

        assert_eq!(
            fx.emitter.recorded_events(),
            vec![
                EmittedEvent::KeyListenerBackend("xinput".into()),
                EmittedEvent::KeyListenerBackend("evtest".into()),
                EmittedEvent::KeyListenerBackend("".into()),
            ]
        );
    }
}
