//! PipelineBuilder — 组件构造。

use std::sync::{Arc, Mutex};

use crate::error::{FatalError, PipelineError};
use crate::key_listener::KeyListener;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::Recorder;
use crate::transcriber::Transcriber;

use super::context::PipelineContext;

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
        Box::new(crate::recorder::PlatformRecorder::new(
            self.cfg.recorder.sample_rate,
        ))
    }

    /// Build transcriber from config.
    ///
    /// Returns error if model not found (local engine) or API initialization fails.
    pub fn build_transcriber(&self) -> Result<Box<dyn Transcriber>, PipelineError> {
        let cfg = &self.cfg.transcriber;

        let model_path = if cfg.engine == "local" {
            match crate::model::resolve_model_path(&cfg.model) {
                Some(p) => p.to_string_lossy().to_string(),
                None => {
                    return Err(PipelineError::Fatal(FatalError::ModelNotFound {
                        model: cfg.model.clone(),
                        searched: vec![dirs::config_dir().unwrap_or_default().join("altgo/models")],
                    }));
                }
            }
        } else {
            cfg.model.clone()
        };

        let transcriber: Box<dyn Transcriber> = match cfg.engine.as_str() {
            "local" => Box::new(crate::whisper_server::ResidentWhisper::new(
                model_path,
                cfg.language.clone(),
                cfg.whisper_path.clone(),
                cfg.temperature,
                cfg.threads,
                cfg.beam_size,
                cfg.timeout,
            )),
            "mimo" => {
                let base_url = if cfg.api_base_url.is_empty() || cfg.api_base_url.contains("openai.com") {
                    "https://api.xiaomimimo.com/v1".to_string()
                } else {
                    cfg.api_base_url.clone()
                };
                let api = crate::transcriber::MimoAsr::new(
                    cfg.api_key.clone(),
                    base_url,
                    cfg.language.clone(),
                    cfg.timeout,
                )
                .map_err(PipelineError::fatal_transcriber)?;
                Box::new(api)
            }
            _ => {
                let api = crate::transcriber::WhisperApi::new(
                    cfg.api_key.clone(),
                    cfg.api_base_url.clone(),
                    cfg.model.clone(),
                    cfg.language.clone(),
                    cfg.temperature,
                    cfg.prompt.clone(),
                    cfg.timeout,
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
    /// 通过 `LLMFormatter::from_config_with_sources` 共享工厂构造，确保与
    /// IPC handler（`cmd::polish_history_entry`）走同一条 prompt source chain。
    pub fn build_polisher(&self) -> Result<LLMFormatter, PipelineError> {
        LLMFormatter::from_config_with_sources(&self.cfg).map_err(PipelineError::fatal_polisher)
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

    /// Build the full pipeline context from configuration.
    pub fn build_context(&self) -> Result<PipelineContext, PipelineError> {
        let recorder = self.build_recorder();
        let transcriber = self.build_transcriber()?;
        let formatter = self.build_polisher()?;
        let polish_level = self.polish_level();
        let listener = self.build_key_listener()?;

        Ok(PipelineContext {
            recorder,
            transcriber,
            formatter,
            polish_level,
            listener: Mutex::new(Some(listener)),
            long_press_threshold: self.cfg.key_listener.long_press_threshold,
            double_click_interval: self.cfg.key_listener.double_click_interval,
            min_press_duration: self.cfg.key_listener.min_press_duration,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polisher::PolishLevel;
    use std::sync::Arc;

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

    // 端到端入口测试：`run()` 必须在 build_context 失败时把错误上报到 sink。
    // 故障点属于 builder（构建上下文），因此下沉到本模块。
    #[tokio::test]
    async fn run_reports_error_when_context_build_fails() {
        use crate::voice_pipeline::sink::TranscriptionResult;
        use std::sync::Mutex;

        struct ErrorSink {
            errors: Arc<Mutex<Vec<String>>>,
        }
        impl crate::voice_pipeline::sink::PipelineSink for ErrorSink {
            fn on_status_change(&self, _: crate::pipeline_controller::PipelineStatus) {}
            fn on_error(&self, msg: &str) {
                self.errors.lock().unwrap().push(msg.to_string());
            }
            fn on_transcription_result(&self, _: &TranscriptionResult) {}
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
        super::super::run(Arc::new(cfg), stop_rx, sink).await;
        assert!(!errors.lock().unwrap().is_empty());
    }
}
