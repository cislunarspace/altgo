//! Voice Pipeline — deep module that owns the full voice-to-text pipeline.
//!
//! Merges the previous `pipeline_orchestrator`, `pipeline_builder`,
//! `pipeline_context`, and `pipeline_command_handler` into one module
//! behind a small interface. `PipelineSink` remains the primary seam
//! (production Tauri sink and test mocks); `PipelineEventHandler` stays
//! a separate adapter for clipboard/history.
//!
//! Public surface:
//! - `run(cfg, stop_rx, sink)` — entry point (builds context + runs event loop)
//! - `PipelineBuilder` — construct components individually (testable)
//! - `PipelineContext` — owns components, exposes `run(stop_rx, sink)`

use std::sync::Arc;
use std::sync::Mutex;

use crate::error::{FatalError, PipelineError};
use crate::history::HistoryStore;
use crate::key_listener::{KeyListener, KeyListenerConfig};
use crate::output::Output;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::Recorder;
use crate::state_machine::{Command, Machine};
use crate::transcriber::Transcriber;

// ---------------------------------------------------------------------------
// Shared types and sink seam
// ---------------------------------------------------------------------------

/// 管道处理结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineOutput {
    /// 处理后的文本（润色成功时为润色文本，否则为原始转写文本）
    pub text: String,
    /// 原始转写文本（润色前）
    pub raw_text: String,
    /// 润色是否失败
    pub polish_failed: bool,
}

/// 管道事件接收器。
///
/// 所有方法均为同步——实现方内部处理异步操作（如 `tokio::spawn`）。
/// 实现方必须是 `Send + Sync + 'static`，以支持跨线程使用。
pub trait PipelineSink: Send + Sync + 'static {
    /// 管道状态变化（idle / recording / processing / done / stopped）。
    fn on_status_change(&self, status: &str);

    /// 管道错误。
    fn on_error(&self, message: &str);

    /// 转写+润色完成，输出结果。
    fn on_transcription_result(&self, output: &PipelineOutput);

    /// 转写/润色进度更新。`phase` 为 `"transcribe"` / `"polish"` / `"done"`，
    /// `fraction` 为 0–1 或 `None`（不确定进度）。
    fn on_progress(&self, phase: &str, fraction: Option<f32>);

    /// 按键监听后端已启动（如 `"xinput"` / `"evtest"`）。
    fn on_key_listener_backend(&self, backend: &str);
}

// ---------------------------------------------------------------------------
// PipelineBuilder — component construction from config
// ---------------------------------------------------------------------------

/// Builds pipeline components from configuration.
pub struct PipelineBuilder {
    cfg: Arc<crate::config::Config>,
}

impl PipelineBuilder {
    pub fn new(cfg: Arc<crate::config::Config>) -> Self {
        Self { cfg }
    }

    /// Build recorder from config.
    pub fn build_recorder(&self) -> Box<dyn Recorder> {
        let recorder_cfg = crate::recorder::RecorderConfig::from(&*self.cfg);
        Box::new(crate::recorder::PlatformRecorder::new(
            recorder_cfg.sample_rate,
            recorder_cfg.channels,
        ))
    }

    /// Build transcriber from config.
    ///
    /// Returns error if model not found (local engine) or API initialization fails.
    pub fn build_transcriber(&self) -> Result<Box<dyn Transcriber>, PipelineError> {
        let transcriber_cfg = crate::transcriber::TranscriberConfig::from(&*self.cfg);

        let model_path = if transcriber_cfg.engine == "local" {
            match crate::model::resolve_model_path(&transcriber_cfg.model) {
                Some(p) => p.to_string_lossy().to_string(),
                None => {
                    return Err(PipelineError::Fatal(FatalError::ModelNotFound {
                        model: transcriber_cfg.model.clone(),
                        searched: vec![dirs::config_dir().unwrap_or_default().join("altgo/models")],
                    }));
                }
            }
        } else {
            transcriber_cfg.model.clone()
        };

        let transcriber: Box<dyn Transcriber> = match transcriber_cfg.engine.as_str() {
            "local" => {
                // 常驻 whisper-server：模型一次性载入内存，之后每句话只发本地 HTTP，
                // 省掉「每次重载模型」的成本。spawn 失败时内部自动回退到一次性 whisper-cli。
                Box::new(crate::whisper_server::ResidentWhisper::new(
                    model_path,
                    transcriber_cfg.language.clone(),
                    transcriber_cfg.whisper_path.clone(),
                    transcriber_cfg.temperature,
                    transcriber_cfg.threads,
                    transcriber_cfg.beam_size,
                    transcriber_cfg.timeout,
                ))
            }
            _ => {
                let api = crate::transcriber::WhisperApi::new(
                    transcriber_cfg.api_key.clone(),
                    transcriber_cfg.api_base_url.clone(),
                    transcriber_cfg.model.clone(),
                    transcriber_cfg.language.clone(),
                    transcriber_cfg.temperature,
                    transcriber_cfg.prompt.clone(),
                    transcriber_cfg.timeout,
                )
                .map_err(PipelineError::fatal_transcriber)?;
                Box::new(api)
            }
        };

        Ok(transcriber)
    }

    /// Build polisher from config.
    ///
    /// Returns error if protocol is unknown or HTTP client fails to initialize.
    pub fn build_polisher(&self) -> Result<LLMFormatter, PipelineError> {
        let polisher_cfg = crate::polisher::PolisherConfig::from(&*self.cfg);
        let formatter =
            LLMFormatter::from_config(&polisher_cfg).map_err(PipelineError::fatal_polisher)?;

        // Build prompt source chain: PromptStore → Custom → Hardcoded
        let mut sources: Vec<Box<dyn crate::polisher::SystemPromptSource>> = Vec::new();

        let prompts_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("resources/prompts")))
            .or_else(|| Some(std::path::PathBuf::from("resources/prompts")));

        if let Some(dir) = prompts_dir {
            if dir.exists() {
                let store = crate::prompt_store::PromptStore::new(dir);
                if let Err(e) = store.ensure_loaded() {
                    tracing::warn!(error = %e, "failed to load prompts from PromptStore, using fallback");
                } else {
                    tracing::info!("PromptStore loaded successfully");
                    sources.push(Box::new(crate::polisher::PromptStoreSource::new(store)));
                }
            } else {
                tracing::debug!("prompts directory not found, using hardcoded prompts");
            }
        }

        if !self.cfg.polisher.system_prompt.is_empty() {
            sources.push(Box::new(crate::polisher::CustomPromptSource::new(
                self.cfg.polisher.system_prompt.clone(),
            )));
        }

        let fallback = Box::new(crate::polisher::HardcodedPromptSource::new(
            polisher_cfg.language.clone(),
        ));
        let prompt_source: Box<dyn crate::polisher::SystemPromptSource> = if sources.is_empty() {
            fallback
        } else {
            Box::new(crate::polisher::FallbackSource::new(sources, fallback))
        };

        Ok(formatter.with_prompt_source(prompt_source))
    }

    /// Build key listener from config.
    ///
    /// Returns a boxed trait object for platform-independent use in the pipeline.
    pub fn build_key_listener(&self) -> Result<Box<dyn KeyListener>, PipelineError> {
        let listener =
            crate::key_listener::PlatformListener::new(&self.cfg.key_listener).map_err(|e| {
                PipelineError::Fatal(FatalError::KeyListenerFailed {
                    backend: "platform".to_string(),
                    reason: e.to_string(),
                })
            })?;
        Ok(Box::new(listener))
    }

    /// Get polish level from config.
    pub fn polish_level(&self) -> PolishLevel {
        PolishLevel::effective(&self.cfg.polisher.level)
    }

    /// Get key listener config for state machine setup.
    pub fn key_listener_config(&self) -> KeyListenerConfig {
        KeyListenerConfig::from(&*self.cfg)
    }

    /// Build the full pipeline context from configuration.
    pub fn build_context(&self) -> Result<PipelineContext, PipelineError> {
        let recorder = self.build_recorder();
        let transcriber = self.build_transcriber()?;
        let formatter = self.build_polisher()?;
        let polish_level = self.polish_level();
        let key_listener_config = self.key_listener_config();
        let listener = self.build_key_listener()?;

        Ok(PipelineContext {
            recorder,
            transcriber,
            formatter,
            polish_level,
            listener: Mutex::new(Some(listener)),
            long_press_threshold: key_listener_config.long_press_threshold,
            double_click_interval: key_listener_config.double_click_interval,
            min_press_duration: key_listener_config.min_press_duration,
        })
    }
}

// ---------------------------------------------------------------------------
// PipelineContext — owns components, runs event loop
// ---------------------------------------------------------------------------

/// Owns all components needed while the pipeline runs.
pub struct PipelineContext {
    recorder: Box<dyn Recorder>,
    transcriber: Box<dyn Transcriber>,
    formatter: LLMFormatter,
    polish_level: PolishLevel,
    listener: Mutex<Option<Box<dyn KeyListener>>>,
    // 状态机参数
    long_press_threshold: std::time::Duration,
    double_click_interval: std::time::Duration,
    min_press_duration: std::time::Duration,
}

impl PipelineContext {
    /// Run the pipeline event loop until `stop_rx` fires.
    pub async fn run(self, stop_rx: tokio::sync::oneshot::Receiver<()>, sink: impl PipelineSink) {
        let mut recorder = self.recorder;
        let transcriber = self.transcriber;
        let formatter = self.formatter;
        let polish_level = self.polish_level;
        // Wrap the sink in an Arc so handlers can keep using it after the
        // async task lifecycle (and so progress forwarders can hold it).
        let sink: Arc<dyn PipelineSink> = Arc::new(sink);

        let mut listener: Box<dyn KeyListener> = match self.listener.lock().unwrap().take() {
            Some(l) => l,
            None => {
                sink.on_error("pipeline context already used");
                return;
            }
        };

        let (mut key_events, key_backend): (
            tokio::sync::mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
            &'static str,
        ) = match listener.start() {
            Ok(pair) => pair,
            Err(e) => {
                sink.on_error(&format!("key listener start: {}", e));
                return;
            }
        };
        tracing::info!(backend = key_backend, "key listener active");
        sink.on_key_listener_backend(key_backend);

        // 创建状态机，直接集成到主循环
        let mut machine = Machine::new(
            self.long_press_threshold,
            self.double_click_interval,
            self.min_press_duration,
        );
        let mut deadline: Option<tokio::time::Instant> = None;

        sink.on_status_change("idle");

        let mut stop_rx = stop_rx;
        loop {
            tokio::select! {
                // 按键事件
                Some(event) = key_events.recv() => {
                    if let Some(cmd) = machine.process(event) {
                        match cmd {
                            Command::StartRecord => {
                                let _ = handle_start_record(&mut *recorder, &*sink);
                            }
                            Command::StopRecord => {
                                handle_stop_record(
                                    &mut *recorder,
                                    &*transcriber,
                                    &formatter,
                                    polish_level,
                                    sink.clone(),
                                )
                                .await;
                            }
                        }
                    }
                    // 更新截止时间
                    deadline = machine.next_deadline().map(|d| d.into());
                }
                // 超时事件
                _ = async { tokio::time::sleep_until(deadline.unwrap()).await }, if deadline.is_some() => {
                    if let Some(cmd) = machine.poll_timeout() {
                        match cmd {
                            Command::StartRecord => {
                                let _ = handle_start_record(&mut *recorder, &*sink);
                            }
                            Command::StopRecord => {
                                handle_stop_record(
                                    &mut *recorder,
                                    &*transcriber,
                                    &formatter,
                                    polish_level,
                                    sink.clone(),
                                )
                                .await;
                            }
                        }
                    }
                    // 更新截止时间
                    deadline = machine.next_deadline().map(|d| d.into());
                }
                // 停止信号
                _ = &mut stop_rx => {
                    tracing::info!("pipeline stop requested");
                    break;
                }
            }
        }

        sink.on_status_change("stopped");
        tracing::info!("pipeline stopped");
    }
}

// ---------------------------------------------------------------------------
// Command handlers — StartRecord and StopRecord business logic
// ---------------------------------------------------------------------------

/// Handle StartRecord command: start recording and notify sink.
pub fn handle_start_record(
    recorder: &mut dyn Recorder,
    sink: &(impl PipelineSink + ?Sized),
) -> Result<(), String> {
    tracing::info!("recording started");
    recorder
        .start_recording()
        .map_err(|e: crate::error::RecorderError| {
            tracing::error!(error = %e, "failed to start recording");
            e.to_string()
        })?;
    sink.on_status_change("recording");
    Ok(())
}

/// Handle StopRecord command: stop recording, process audio, notify sink.
pub async fn handle_stop_record(
    recorder: &mut dyn Recorder,
    transcriber: &dyn Transcriber,
    formatter: &LLMFormatter,
    polish_level: PolishLevel,
    sink: Arc<dyn PipelineSink>,
) {
    tracing::info!("recording stopped, processing...");
    sink.on_status_change("processing");

    let wav_data: Vec<u8> = match recorder.stop_recording() {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(error = %e, "failed to stop recording");
            sink.on_status_change("idle");
            return;
        }
    };

    sink.on_progress("transcribe", None);

    // Bridge the trait's Arc<dyn Fn> progress callback to the sink — the
    // callback must own its data, so we run a small forwarder task that
    // listens to an mpsc channel and invokes the callback.
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
    let progress_cb: Arc<dyn Fn(f32) + Send + Sync> = Arc::new(move |fr: f32| {
        let _ = progress_tx.send(fr);
    });
    let forwarder_sink = sink.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(fr) = progress_rx.recv().await {
            forwarder_sink.on_progress("transcribe", Some(fr));
        }
    });

    let transcribe_result = transcriber.transcribe(&wav_data, progress_cb).await;
    let _ = forwarder.await;
    let result = match transcribe_result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "transcription failed");
            sink.on_error(&format!("transcription: {}", e));
            return;
        }
    };

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        sink.on_progress("done", Some(1.0));
        sink.on_transcription_result(&PipelineOutput {
            text: String::new(),
            raw_text: String::new(),
            polish_failed: false,
        });
        return;
    }

    sink.on_progress("polish", None);

    let mut polish_failed = false;
    let raw_text = result.text.clone();
    let polished = match formatter.polish(&raw_text, polish_level).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "polish failed, using raw text");
            polish_failed = true;
            raw_text.clone()
        }
    };

    tracing::info!(text = %polished, "polished");

    sink.on_progress("done", Some(1.0));

    let output = PipelineOutput {
        text: polished,
        raw_text,
        polish_failed,
    };
    sink.on_transcription_result(&output);
}

// ---------------------------------------------------------------------------
// Result processing — text selection, clipboard, history
// ---------------------------------------------------------------------------

/// Result of processing a transcription event.
#[derive(Debug, Clone)]
pub struct ProcessedResult {
    /// Text that was written to clipboard and should be shown.
    pub text: String,
    /// Whether history was appended successfully.
    pub history_appended: bool,
}

/// Select which text to use based on preferences and polish status.
pub fn select_text(prefer_polished: bool, output: &PipelineOutput) -> String {
    if prefer_polished && !output.polish_failed && !output.text.trim().is_empty() {
        output.text.clone()
    } else {
        output.raw_text.clone()
    }
}

/// Process a transcription result: select text, write clipboard, append history.
///
/// Returns `None` if the transcription was empty (no action taken).
pub async fn process_transcription_result(
    output: &PipelineOutput,
    prefer_polished: bool,
    output_adapter: &dyn Output,
    history_store: &HistoryStore,
) -> Option<ProcessedResult> {
    if output.raw_text.is_empty() {
        return None;
    }

    let text_to_use = select_text(prefer_polished, output);

    // Write to clipboard (blocking I/O; caller is already in an async context)
    let text_clone = text_to_use.clone();
    let output_handle = output_adapter.clone_box();
    let clipboard_ok =
        tokio::task::spawn_blocking(move || output_handle.write_clipboard(&text_clone))
            .await
            .ok()
            .and_then(|r| r.ok())
            .is_some();
    if !clipboard_ok {
        tracing::warn!("failed to write clipboard");
    }

    // Append to history
    let raw = output.raw_text.clone();
    let display = text_to_use.clone();
    let store = history_store.clone();
    let history_appended = tokio::task::spawn_blocking(move || store.append(raw, display))
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some();

    if !history_appended {
        tracing::warn!("failed to append transcription history");
    }

    Some(ProcessedResult {
        text: text_to_use,
        history_appended,
    })
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

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
    use crate::polisher::PolisherConfig;
    use crate::recorder::PlatformRecorder;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    // -- PipelineBuilder tests ---------------------------------------------

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
                crate::error::PolisherError::UnknownProtocol { .. }
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

    // -- PipelineContext tests ---------------------------------------------

    struct FakeListener {
        backend: &'static str,
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

    fn test_polisher_config() -> PolisherConfig {
        PolisherConfig {
            api_key: "test-key".to_string(),
            api_base_url: "http://localhost".to_string(),
            model: "test-model".to_string(),
            protocol: "openai".to_string(),
            max_tokens: 256,
            temperature: 0.0,
            system_prompt: String::new(),
            timeout: std::time::Duration::from_secs(10),
            level: "none".to_string(),
            language: "en".to_string(),
        }
    }

    fn make_context(listener: Option<Box<dyn KeyListener>>) -> PipelineContext {
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
            formatter: LLMFormatter::from_config(&test_polisher_config()).unwrap(),
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

    // -- Command handler tests ---------------------------------------------

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

    struct FakeRecorder {
        recording: std::sync::atomic::AtomicBool,
        audio: Vec<u8>,
    }

    impl FakeRecorder {
        fn new(audio: Vec<u8>) -> Self {
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

    // -- Result processing tests ---------------------------------------------

    struct FakeOutput {
        clipboard_writes: Arc<Mutex<Vec<String>>>,
    }

    impl FakeOutput {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
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

    // -- Pipeline orchestrator entry-point test ----------------------------

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
