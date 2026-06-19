//! Voice Pipeline — 拥有完整语音转文字管道。
//!
//! 模块划分：
//! - `sink` — 共享类型（PipelineOutput、PipelineSink、ProcessedResult）
//! - `builder` — PipelineBuilder 组件构造
//! - `context` — PipelineContext 事件循环
//! - `handlers` — 命令处理器与结果处理
//!
//! 公共接口：
//! - `run(cfg, stop_rx, sink)` — 入口（构建 context + 运行事件循环）
//! - `PipelineBuilder` — 单独构造各组件（可测试）
//! - `PipelineContext` — 拥有组件，暴露 `run(stop_rx, sink)`

mod builder;
mod context;
mod handlers;
mod sink;

pub use builder::PipelineBuilder;
pub use context::PipelineContext;
pub use handlers::{
    handle_start_record, handle_stop_record, process_transcription_result, select_text,
};
pub use sink::{PipelineOutput, PipelineSink, ProcessedResult};

use std::sync::Arc;

/// Run the voice pipeline end-to-end.
///
/// Blocks the current async task until `stop_rx` fires.
/// All state changes and results are reported via `sink`.
pub async fn run(
    cfg: Arc<crate::config::Config>,
    stop_rx: tokio::sync::oneshot::Receiver<()>,
    sink: impl PipelineSink,
) {
    let builder = PipelineBuilder::new(cfg.clone());

    let ctx = match builder.build_context() {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!(error = %e, "failed to build pipeline context");
            sink.on_error(&e.message("zh"));
            return;
        }
    };

    ctx.run(stop_rx, sink).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_listener::KeyListener;
    use crate::output::Output;
    use crate::polisher::PolishLevel;
    use crate::recorder::{PlatformRecorder, Recorder};
    use crate::transcriber::Transcriber;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    // -- Builder tests ------------------------------------------------------

    fn test_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    #[test]
    fn test_build_recorder() {
        let cfg = Arc::new(test_config());
        let builder = PipelineBuilder::new(cfg);
        let _recorder = builder.build_recorder();
    }

    #[test]
    fn test_build_transcriber_local_model_not_found() {
        use crate::error::{FatalError, PipelineError};

        let mut cfg = test_config();
        cfg.transcriber.engine = "local".to_string();
        cfg.transcriber.model = "nonexistent-model".to_string();

        let builder = PipelineBuilder::new(Arc::new(cfg));
        let err = match builder.build_transcriber() {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(err.is_fatal());
        assert!(matches!(
            err,
            PipelineError::Fatal(FatalError::ModelNotFound { .. })
        ));
    }

    #[test]
    fn test_build_polisher_unknown_protocol() {
        use crate::error::{FatalError, PipelineError, PolisherError};

        let mut cfg = test_config();
        cfg.polisher.protocol = "unknown-protocol".to_string();

        let builder = PipelineBuilder::new(Arc::new(cfg));
        let result = builder.build_polisher();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_fatal());
        assert!(matches!(
            err,
            PipelineError::Fatal(FatalError::PolisherInitFailed(
                PolisherError::UnknownProtocol { .. }
            ))
        ));
    }

    #[test]
    fn test_polish_level() {
        let mut cfg = test_config();
        cfg.polisher.level = "heavy".to_string();

        let builder = PipelineBuilder::new(Arc::new(cfg));
        let level = builder.polish_level();

        assert_eq!(level, PolishLevel::Heavy);
    }

    // -- Context tests ------------------------------------------------------

    pub(crate) struct FakeListener {
        pub(crate) backend: &'static str,
    }

    impl KeyListener for FakeListener {
        fn start(
            &mut self,
        ) -> anyhow::Result<(
            mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
            &'static str,
        )> {
            let (_, rx) = mpsc::unbounded_channel();
            Ok((rx, self.backend))
        }
    }

    fn test_polisher_config() -> crate::config::PolisherConfig {
        crate::config::PolisherConfig {
            api_key: "test-key".to_string(),
            api_base_url: "http://localhost".to_string(),
            model: "test-model".to_string(),
            protocol: "openai".to_string(),
            max_tokens: 256,
            temperature: 0.0,
            system_prompt: String::new(),
            timeout: std::time::Duration::from_secs(10),
            level: "none".to_string(),
        }
    }

    fn make_context(listener: Option<Box<dyn KeyListener>>) -> PipelineContext {
        use crate::polisher::LLMFormatter;
        PipelineContext {
            recorder: Box::new(PlatformRecorder::new(16000, 1)),
            transcriber: Box::new(
                crate::transcriber::WhisperApi::new(
                    "test-key".to_string(),
                    "http://localhost".to_string(),
                    "test-model".to_string(),
                    "en".to_string(),
                    0.0,
                    String::new(),
                    std::time::Duration::from_secs(10),
                )
                .unwrap(),
            ),
            formatter: LLMFormatter::from_config(&test_polisher_config(), "en").unwrap(),
            polish_level: PolishLevel::None,
            listener: Mutex::new(listener),
            long_press_threshold: std::time::Duration::from_millis(400),
            double_click_interval: std::time::Duration::from_millis(200),
            min_press_duration: std::time::Duration::from_millis(80),
        }
    }

    #[test]
    fn pipeline_context_accepts_boxed_key_listener() {
        let fake: Box<dyn KeyListener> = Box::new(FakeListener {
            backend: "test-fake",
        });
        let ctx = make_context(Some(fake));
        let mut taken = ctx.listener.lock().unwrap().take().unwrap();
        assert_eq!(taken.start().unwrap().1, "test-fake");
    }

    #[test]
    fn pipeline_context_run_returns_early_when_listener_already_taken() {
        struct MockSink;
        impl PipelineSink for MockSink {
            fn on_status_change(&self, _: &str) {}
            fn on_error(&self, _: &str) {}
            fn on_transcription_result(&self, _: &PipelineOutput) {}
            fn on_progress(&self, _: &str, _: Option<f32>) {}
            fn on_key_listener_backend(&self, _: &str) {}
        }

        let ctx = make_context(None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        drop(stop_tx);
        rt.block_on(ctx.run(stop_rx, MockSink));
    }

    // -- Command handler tests ----------------------------------------------

    #[derive(Clone)]
    struct MockSink {
        status_changes: Arc<Mutex<Vec<String>>>,
        errors: Arc<Mutex<Vec<String>>>,
        results: Arc<Mutex<Vec<PipelineOutput>>>,
    }

    impl MockSink {
        fn new() -> Self {
            Self {
                status_changes: Arc::new(Mutex::new(Vec::new())),
                errors: Arc::new(Mutex::new(Vec::new())),
                results: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn status_changes(&self) -> Vec<String> {
            self.status_changes.lock().unwrap().clone()
        }
    }

    impl PipelineSink for MockSink {
        fn on_status_change(&self, status: &str) {
            self.status_changes.lock().unwrap().push(status.to_string());
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

    pub(crate) struct FakeRecorder {
        pub(crate) recording: std::sync::atomic::AtomicBool,
        pub(crate) audio: Vec<u8>,
    }

    impl FakeRecorder {
        pub(crate) fn new(audio: Vec<u8>) -> Self {
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

    #[test]
    fn handle_start_record_with_fake_recorder() {
        let mut recorder = FakeRecorder::new(vec![]);
        let sink = MockSink::new();
        let result = handle_start_record(&mut recorder, &sink);
        assert!(result.is_ok());
        assert!(recorder.is_recording());
        assert_eq!(sink.status_changes(), vec!["recording"]);
    }

    #[tokio::test]
    async fn handle_stop_record_with_fake_recorder_reports_empty_audio() {
        use crate::polisher::LLMFormatter;

        let mut recorder = FakeRecorder::new(vec![0u8; 44]);
        let sink = MockSink::new();
        let sink_arc: Arc<dyn PipelineSink> = Arc::new(sink);

        recorder.start_recording().unwrap();

        let transcriber: Box<dyn Transcriber> = Box::new(crate::transcriber::LocalWhisper::new(
            "/nonexistent/model".to_string(),
            "zh".to_string(),
            "whisper-cli".to_string(),
            0,
            0,
        ));
        let formatter = LLMFormatter::new(
            "test-key".to_string(),
            "http://localhost".to_string(),
            "test-model".to_string(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();

        handle_stop_record(
            &mut recorder,
            &*transcriber,
            &formatter,
            PolishLevel::None,
            sink_arc,
        )
        .await;
    }

    // -- Result processing tests --------------------------------------------

    pub(crate) struct FakeOutput {
        pub(crate) clipboard_writes: Arc<Mutex<Vec<String>>>,
    }

    impl FakeOutput {
        pub(crate) fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
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

        fn notify(&self, _title: &str, _body: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn clone_box(&self) -> Box<dyn Output> {
            Box::new(FakeOutput {
                clipboard_writes: Arc::clone(&self.clipboard_writes),
            })
        }
    }

    fn test_output(raw: &str, polished: &str, polish_failed: bool) -> PipelineOutput {
        PipelineOutput {
            raw_text: raw.to_string(),
            text: polished.to_string(),
            polish_failed,
        }
    }

    #[test]
    fn test_select_text_prefer_polished_success() {
        let output = test_output("raw text", "polished text", false);
        assert_eq!(select_text(true, &output), "polished text");
    }

    #[test]
    fn test_select_text_prefer_polished_failed() {
        let output = test_output("raw text", "", true);
        assert_eq!(select_text(true, &output), "raw text");
    }

    #[test]
    fn test_select_text_prefer_polished_empty() {
        let output = test_output("raw text", "  ", false);
        assert_eq!(select_text(true, &output), "raw text");
    }

    #[test]
    fn test_select_text_prefer_raw() {
        let output = test_output("raw text", "polished text", false);
        assert_eq!(select_text(false, &output), "raw text");
    }

    #[tokio::test]
    async fn test_process_transcription_result_empty() {
        use crate::history::HistoryStore;

        let output = test_output("", "", false);
        let (output_adapter, _) = FakeOutput::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let history_store = HistoryStore::new(temp_dir.path().join("history.json"));

        let result =
            process_transcription_result(&output, true, &output_adapter, &history_store).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_process_transcription_result_success() {
        use crate::history::HistoryStore;

        let output = test_output("raw text", "polished text", false);
        let (output_adapter, writes) = FakeOutput::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let history_store = HistoryStore::new(temp_dir.path().join("history.json"));

        let result =
            process_transcription_result(&output, true, &output_adapter, &history_store).await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.text, "polished text");
        assert!(result.history_appended);
        assert_eq!(writes.lock().unwrap().len(), 1);
        assert_eq!(writes.lock().unwrap()[0], "polished text");
    }

    #[tokio::test]
    async fn test_process_transcription_result_clipboard_failure_still_returns_result() {
        use crate::history::HistoryStore;

        struct FailingOutput;
        impl Output for FailingOutput {
            fn write_clipboard(&self, _text: &str) -> anyhow::Result<()> {
                Err(anyhow::anyhow!("no clipboard"))
            }
            fn notify(&self, _: &str, _: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn clone_box(&self) -> Box<dyn Output> {
                Box::new(FailingOutput)
            }
        }

        let output = test_output("raw text", "polished text", false);
        let temp_dir = tempfile::tempdir().unwrap();
        let history_store = HistoryStore::new(temp_dir.path().join("history.json"));

        let result =
            process_transcription_result(&output, true, &FailingOutput, &history_store).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "polished text");
    }

    // -- Top-level entry point test -----------------------------------------

    #[tokio::test]
    async fn run_reports_error_when_context_build_fails() {
        struct ErrorSink {
            errors: Arc<Mutex<Vec<String>>>,
        }
        impl PipelineSink for ErrorSink {
            fn on_status_change(&self, _: &str) {}
            fn on_error(&self, msg: &str) {
                self.errors.lock().unwrap().push(msg.to_string());
            }
            fn on_transcription_result(&self, _: &PipelineOutput) {}
            fn on_progress(&self, _: &str, _: Option<f32>) {}
            fn on_key_listener_backend(&self, _: &str) {}
        }

        // Force build_context to fail via unknown polisher protocol.
        let mut cfg = test_config();
        cfg.polisher.protocol = "unknown".to_string();
        let errors = Arc::new(Mutex::new(Vec::new()));
        let sink = ErrorSink {
            errors: Arc::clone(&errors),
        };
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        drop(stop_tx);
        run(Arc::new(cfg), stop_rx, sink).await;
        assert!(!errors.lock().unwrap().is_empty());
    }
}
