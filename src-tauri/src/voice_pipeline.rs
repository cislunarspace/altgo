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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{FatalError, PipelineError};
use crate::key_listener::{KeyListener, KeyListenerConfig};
use crate::pipeline_sink::PipelineSink;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::Recorder;
use crate::state_machine::{Command, Machine};
use crate::transcriber::Transcriber;

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
    pub fn build_transcriber(&self) -> Result<Transcriber, PipelineError> {
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

        let transcriber = match transcriber_cfg.engine.as_str() {
            "local" => {
                // 常驻 whisper-server：模型一次性载入内存，之后每句话只发本地 HTTP，
                // 省掉「每次重载模型」的成本。spawn 失败时内部自动回退到一次性 whisper-cli。
                Transcriber::Resident(crate::whisper_server::ResidentWhisper::new(
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
                .map_err(|e| {
                    PipelineError::Fatal(FatalError::TranscriberInitFailed(
                        crate::error::TranscriberError::HttpError(e.to_string()),
                    ))
                })?;
                Transcriber::Api(api)
            }
        };

        Ok(transcriber)
    }

    /// Build polisher from config.
    ///
    /// Returns error if protocol is unknown or HTTP client fails to initialize.
    pub fn build_polisher(&self) -> Result<LLMFormatter, PipelineError> {
        let polisher_cfg = crate::polisher::PolisherConfig::from(&*self.cfg);
        let formatter = LLMFormatter::from_config(&polisher_cfg)
            .map_err(|e| PipelineError::Fatal(FatalError::PolisherInitFailed(e)))?;

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
    pub fn build_key_listener(
        &self,
    ) -> Result<Box<dyn KeyListener>, PipelineError> {
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
        let poll_interval_ms = key_listener_config.poll_interval_ms;
        let listener = self.build_key_listener()?;

        Ok(PipelineContext {
            recorder,
            transcriber,
            formatter,
            polish_level,
            poll_running: Arc::new(AtomicBool::new(true)),
            key_listener_config,
            poll_interval_ms,
            listener: Mutex::new(Some(listener)),
        })
    }
}

// ---------------------------------------------------------------------------
// PipelineContext — owns components, runs event loop
// ---------------------------------------------------------------------------

/// Owns all components needed while the pipeline runs.
pub struct PipelineContext {
    recorder: Box<dyn Recorder>,
    transcriber: Transcriber,
    formatter: LLMFormatter,
    polish_level: PolishLevel,
    poll_running: Arc<AtomicBool>,
    key_listener_config: KeyListenerConfig,
    poll_interval_ms: u64,
    listener: Mutex<Option<Box<dyn KeyListener>>>,
}

impl PipelineContext {
    /// Run the pipeline event loop until `stop_rx` fires.
    pub async fn run(
        self,
        stop_rx: tokio::sync::oneshot::Receiver<()>,
        sink: impl PipelineSink,
    ) {
        // Extract fields we need before the async loop borrows self mutably.
        let poll_running = self.poll_running.clone();
        let poll_interval_ms = self.poll_interval_ms;
        let key_listener_config = self.key_listener_config.clone();
        let mut recorder = self.recorder;
        let transcriber = self.transcriber;
        let formatter = self.formatter;
        let polish_level = self.polish_level;

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

        let (raw_key_tx, raw_key_rx) = tokio::sync::mpsc::unbounded_channel();
        let poll_running_for_thread = poll_running.clone();

        std::thread::spawn(move || {
            use tokio::sync::mpsc::error::TryRecvError;
            while poll_running_for_thread.load(Ordering::SeqCst) {
                match key_events.try_recv() {
                    Ok(ev) => {
                        if raw_key_tx.send(ev).is_err() {
                            break;
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                    }
                    Err(TryRecvError::Disconnected) => {
                        tracing::error!("key listener channel closed unexpectedly");
                        break;
                    }
                }
            }
        });

        let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(crate::key_listener::debounce_task(
            raw_key_rx,
            key_tx.clone(),
        ));

        let sm = Machine::new(
            key_listener_config.long_press_threshold,
            key_listener_config.double_click_interval,
            key_listener_config.min_press_duration,
        );
        let mut commands = sm.run(key_rx);

        sink.on_status_change("idle");

        let mut stop_rx = stop_rx;
        loop {
            tokio::select! {
                cmd = commands.recv() => {
                    match cmd {
                        Some(Command::StartRecord) => {
                            let _ = handle_start_record(&mut *recorder, &sink);
                        }
                        Some(Command::StopRecord) => {
                            handle_stop_record(
                                &mut *recorder,
                                &transcriber,
                                &formatter,
                                polish_level,
                                &sink,
                            )
                            .await;
                        }
                        None => break,
                    }
                }
                _ = &mut stop_rx => {
                    tracing::info!("pipeline stop requested");
                    poll_running.store(false, Ordering::SeqCst);
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
    sink: &impl PipelineSink,
) -> Result<(), String> {
    tracing::info!("recording started");
    recorder.start_recording().map_err(|e: anyhow::Error| {
        tracing::error!(error = %e, "failed to start recording");
        e.to_string()
    })?;
    sink.on_status_change("recording");
    Ok(())
}

/// Handle StopRecord command: stop recording, process audio, notify sink.
pub async fn handle_stop_record(
    recorder: &mut dyn Recorder,
    transcriber: &Transcriber,
    formatter: &LLMFormatter,
    polish_level: PolishLevel,
    sink: &impl PipelineSink,
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

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
    let wav_owned = wav_data.to_vec();
    let transcribe_jh = tokio::spawn({
        let t = transcriber.clone();
        async move { t.transcribe(&wav_owned, Some(progress_tx)).await }
    });

    while let Some(fr) = progress_rx.recv().await {
        sink.on_progress("transcribe", Some(fr));
    }

    let result = match transcribe_jh.await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            tracing::error!(error = %e, "transcription failed");
            sink.on_error(&format!("transcription: {}", e));
            return;
        }
        Err(e) => {
            tracing::error!(error = %e, "transcribe task join");
            sink.on_error("transcription: task join");
            return;
        }
    };

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        sink.on_progress("done", Some(1.0));
        sink.on_transcription_result(&crate::pipeline::PipelineOutput {
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

    let output = crate::pipeline::PipelineOutput {
        text: polished,
        raw_text,
        polish_failed,
    };
    sink.on_transcription_result(&output);
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
    use crate::pipeline::PipelineOutput;
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
        let result = builder.build_transcriber();

        assert!(result.is_err());
        let err = result.unwrap_err();
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
            transcriber: Transcriber::Api(crate::transcriber::WhisperApi::new(
                "test-key".to_string(),
                "http://localhost".to_string(),
                "test-model".to_string(),
                "en".to_string(),
                0.0,
                String::new(),
                std::time::Duration::from_secs(10),
            ).unwrap()),
            formatter: LLMFormatter::from_config(&test_polisher_config()).unwrap(),
            polish_level: PolishLevel::None,
            poll_running: Arc::new(AtomicBool::new(true)),
            key_listener_config: KeyListenerConfig {
                key_name: "Alt_R".to_string(),
                linux_evdev_code: None,
                windows_vk: None,
                long_press_threshold: std::time::Duration::from_millis(400),
                double_click_interval: std::time::Duration::from_millis(200),
                debounce_window: std::time::Duration::from_millis(30),
                poll_interval_ms: 10,
                min_press_duration: std::time::Duration::from_millis(80),
            },
            poll_interval_ms: 10,
            listener: Mutex::new(listener),
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
        fn start_recording(&mut self) -> anyhow::Result<()> {
            self.recording.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        fn stop_recording(&self) -> anyhow::Result<Vec<u8>> {
            self.recording.store(false, std::sync::atomic::Ordering::SeqCst);
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

        recorder.start_recording().unwrap();

        let transcriber = Transcriber::Local(crate::transcriber::LocalWhisper::new(
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
            &transcriber,
            &formatter,
            PolishLevel::None,
            &sink,
        )
        .await;

        let statuses = sink.status_changes();
        assert!(statuses.contains(&"processing".to_string()));
        assert!(
            statuses.contains(&"idle".to_string()) || !sink.errors.lock().unwrap().is_empty(),
            "Expected idle status or error"
        );
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
        let sink = ErrorSink { errors: Arc::clone(&errors) };
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        drop(stop_tx);
        run(Arc::new(cfg), stop_rx, sink).await;
        assert!(!errors.lock().unwrap().is_empty());
    }
}
