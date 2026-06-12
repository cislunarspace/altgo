//! Pipeline component builder.
//!
//! Centralizes construction logic for all pipeline components (recorder, transcriber,
//! polisher, key listener). Extracts the 75-line setup block from pipeline_orchestrator
//! into focused, testable builder methods.

#[allow(unused_imports)]
use crate::error::{FatalError, PipelineError, PolisherError, TranscriberError};
use std::sync::{atomic::AtomicBool, Arc};

/// Builds pipeline components from configuration.
pub struct PipelineBuilder {
    cfg: Arc<crate::config::Config>,
}

impl PipelineBuilder {
    pub fn new(cfg: Arc<crate::config::Config>) -> Self {
        Self { cfg }
    }

    /// Build recorder from config.
    pub fn build_recorder(&self) -> crate::recorder::PlatformRecorder {
        let recorder_cfg = crate::recorder::RecorderConfig::from(&*self.cfg);
        crate::recorder::PlatformRecorder::new(recorder_cfg.sample_rate, recorder_cfg.channels)
    }

    /// Build transcriber from config.
    ///
    /// Returns error if model not found (local engine) or API initialization fails.
    pub fn build_transcriber(&self) -> Result<crate::transcriber::Transcriber, PipelineError> {
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
                crate::transcriber::Transcriber::Resident(
                    crate::whisper_server::ResidentWhisper::new(
                        model_path,
                        transcriber_cfg.language.clone(),
                        transcriber_cfg.whisper_path.clone(),
                        transcriber_cfg.temperature,
                        transcriber_cfg.threads,
                        transcriber_cfg.beam_size,
                        transcriber_cfg.timeout,
                    ),
                )
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
                        TranscriberError::HttpError(e.to_string()),
                    ))
                })?;
                crate::transcriber::Transcriber::Api(api)
            }
        };

        Ok(transcriber)
    }

    /// Build polisher from config.
    ///
    /// Returns error if protocol is unknown or HTTP client fails to initialize.
    pub fn build_polisher(&self) -> Result<crate::polisher::LLMFormatter, PipelineError> {
        let polisher_cfg = crate::polisher::PolisherConfig::from(&*self.cfg);
        let mut formatter = crate::polisher::LLMFormatter::from_config(&polisher_cfg)
            .map_err(|e| PipelineError::Fatal(FatalError::PolisherInitFailed(e)))?;

        // Try to load PromptStore from resources/prompts/
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
                    formatter = formatter.with_prompt_store(store);
                }
            } else {
                tracing::debug!("prompts directory not found, using hardcoded prompts");
            }
        }

        Ok(formatter)
    }

    /// Build key listener from config.
    ///
    /// Returns the listener and a receiver for key events.
    pub fn build_key_listener(
        &self,
    ) -> Result<
        (
            crate::key_listener::PlatformListener,
            tokio::sync::mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
            String,
        ),
        PipelineError,
    > {
        let mut listener = crate::key_listener::PlatformListener::new(&self.cfg.key_listener)
            .map_err(|e| {
                PipelineError::Fatal(FatalError::KeyListenerFailed {
                    backend: "unknown".to_string(),
                    reason: e.to_string(),
                })
            })?;

        let (key_events, key_backend): (
            tokio::sync::mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
            &'static str,
        ) = listener.start().map_err(|e| {
            PipelineError::Fatal(FatalError::KeyListenerFailed {
                backend: "unknown".to_string(),
                reason: format!("failed to start: {}", e),
            })
        })?;

        Ok((listener, key_events, key_backend.to_string()))
    }

    /// Get polish level from config.
    pub fn polish_level(&self) -> crate::polisher::PolishLevel {
        crate::polisher::PolishLevel::effective(&self.cfg.polisher.level)
    }

    /// Get key listener config for state machine setup.
    pub fn key_listener_config(&self) -> crate::key_listener::KeyListenerConfig {
        crate::key_listener::KeyListenerConfig::from(&*self.cfg)
    }

    /// Build the full pipeline context from configuration.
    ///
    /// Returns error if any component fails to initialize.
    pub fn build_context(&self) -> Result<crate::pipeline_context::PipelineContext, PipelineError> {
        let recorder = self.build_recorder();
        let transcriber = self.build_transcriber()?;
        let formatter = self.build_polisher()?;
        let polish_level = self.polish_level();
        let key_listener_config = self.key_listener_config();
        let poll_interval_ms = key_listener_config.poll_interval_ms;

        let listener =
            crate::key_listener::PlatformListener::new(&self.cfg.key_listener).map_err(|e| {
                PipelineError::Fatal(FatalError::KeyListenerFailed {
                    backend: "platform".to_string(),
                    reason: e.to_string(),
                })
            })?;

        Ok(crate::pipeline_context::PipelineContext {
            recorder,
            transcriber,
            formatter,
            polish_level,
            poll_running: Arc::new(AtomicBool::new(true)),
            key_listener_config,
            poll_interval_ms,
            listener: std::sync::Mutex::new(Some(listener)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    #[test]
    fn test_build_recorder() {
        let cfg = Arc::new(test_config());
        let builder = PipelineBuilder::new(cfg);
        let _recorder = builder.build_recorder();
        // Recorder construction should not fail
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

        assert_eq!(level, crate::polisher::PolishLevel::Heavy);
    }
}
