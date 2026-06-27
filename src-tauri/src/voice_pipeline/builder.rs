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
            self.cfg.recorder.channels,
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
    pub fn build_polisher(&self) -> Result<LLMFormatter, PipelineError> {
        let formatter =
            LLMFormatter::from_config(&self.cfg.polisher, &self.cfg.transcriber.language)
                .map_err(PipelineError::fatal_polisher)?;

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
            self.cfg.transcriber.language.clone(),
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
