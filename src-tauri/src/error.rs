//! Structured error types for the altgo pipeline.
//!
//! Provides bilingual error messages (Chinese/English) and distinguishes
//! between fatal errors (stop pipeline) and recoverable errors (degrade gracefully).

use std::path::PathBuf;

/// Top-level pipeline error with fatal/recoverable distinction.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("{0}")]
    Fatal(FatalError),

    #[error("{0}")]
    Recoverable(RecoverableError),
}

impl PipelineError {
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Recoverable(_))
    }

    /// Wrap a `TranscriberError` as a fatal `TranscriberInitFailed` (for
    /// construction-time failures) or as a recoverable `TranscriptionFailed`
    /// (for runtime failures). Callers choose which via this method.
    pub fn fatal_transcriber(e: TranscriberError) -> Self {
        Self::Fatal(FatalError::TranscriberInitFailed(e))
    }

    /// Wrap a `PolisherError` as a fatal `PolisherInitFailed` (for construction
    /// time) or as a recoverable `PolishingFailed` (for runtime failures).
    pub fn fatal_polisher(e: PolisherError) -> Self {
        Self::Fatal(FatalError::PolisherInitFailed(e))
    }

    /// Wrap a `RecorderError` as a fatal `RecorderInitFailed`.
    pub fn fatal_recorder(e: RecorderError) -> Self {
        Self::Fatal(FatalError::RecorderInitFailed(e))
    }

    /// Returns a user-facing error message in the specified language.
    pub fn message(&self, lang: &str) -> String {
        match self {
            Self::Fatal(e) => e.message(lang),
            Self::Recoverable(e) => e.message(lang),
        }
    }
}

/// Fatal errors that should stop the pipeline.
#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("Model not found: {model}")]
    ModelNotFound {
        model: String,
        searched: Vec<PathBuf>,
    },

    #[error("API authentication failed: {service} returned {status}")]
    ApiAuthFailed { service: &'static str, status: u16 },

    #[error("Key listener failed to start: {backend} - {reason}")]
    KeyListenerFailed { backend: String, reason: String },

    #[error("Transcriber initialization failed: {0}")]
    TranscriberInitFailed(#[from] TranscriberError),

    #[error("Polisher initialization failed: {0}")]
    PolisherInitFailed(#[from] PolisherError),

    #[error("Recorder initialization failed: {0}")]
    RecorderInitFailed(#[from] RecorderError),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl FatalError {
    pub fn message(&self, lang: &str) -> String {
        match self {
            Self::ModelNotFound { model, searched } => {
                if lang == "zh" {
                    format!(
                        "本地模型未找到（配置值: {:?}）。\n搜索路径: {:?}\n请在 GUI 设置中下载模型，或将 [transcriber] model 设为已下载模型的名称（如 \"base\"）或完整文件路径。",
                        model, searched
                    )
                } else {
                    format!(
                        "Model '{}' not found.\nSearched: {:?}\nDownload it in Settings or set [transcriber] model to an existing model name (e.g., \"base\") or full path.",
                        model, searched
                    )
                }
            }
            Self::ApiAuthFailed { service, status } => {
                if lang == "zh" {
                    format!(
                        "{} API 认证失败（HTTP {}）。请检查 API 密钥配置。",
                        service, status
                    )
                } else {
                    format!(
                        "{} API authentication failed (HTTP {}). Check your API key configuration.",
                        service, status
                    )
                }
            }
            Self::KeyListenerFailed { backend, reason } => {
                if lang == "zh" {
                    format!("按键监听器启动失败（{}）: {}", backend, reason)
                } else {
                    format!("Key listener failed to start ({}): {}", backend, reason)
                }
            }
            Self::TranscriberInitFailed(e) => e.message(lang),
            Self::PolisherInitFailed(e) => e.message(lang),
            Self::RecorderInitFailed(e) => e.message(lang),
            Self::ConfigError(msg) => {
                if lang == "zh" {
                    format!("配置错误: {}", msg)
                } else {
                    format!("Configuration error: {}", msg)
                }
            }
        }
    }
}

/// Recoverable errors that allow graceful degradation.
#[derive(Debug, thiserror::Error)]
pub enum RecoverableError {
    #[error("Transcription failed: {0}")]
    TranscriptionFailed(#[from] TranscriberError),

    #[error("Polishing failed: {0}")]
    PolishingFailed(#[from] PolisherError),

    #[error("Recording failed: {0}")]
    RecordingFailed(#[from] RecorderError),

    #[error("Empty transcription result")]
    EmptyTranscription,

    #[error("Output failed: {0}")]
    OutputFailed(String),
}

impl RecoverableError {
    pub fn message(&self, lang: &str) -> String {
        match self {
            Self::TranscriptionFailed(e) => e.message(lang),
            Self::PolishingFailed(e) => e.message(lang),
            Self::RecordingFailed(e) => e.message(lang),
            Self::EmptyTranscription => {
                if lang == "zh" {
                    "转写结果为空，请重试。".to_string()
                } else {
                    "Transcription returned empty result. Please try again.".to_string()
                }
            }
            Self::OutputFailed(msg) => {
                if lang == "zh" {
                    format!("输出失败: {}", msg)
                } else {
                    format!("Output failed: {}", msg)
                }
            }
        }
    }
}

/// Transcriber-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum TranscriberError {
    #[error("Empty audio data")]
    EmptyAudio,

    #[error("API key not configured")]
    MissingApiKey,

    #[error("Whisper API returned {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("whisper-cli not found at: {path}")]
    WhisperCliNotFound { path: String },

    #[error("whisper-cli failed (exit code {code}): {output}")]
    WhisperCliFailed { code: i32, output: String },

    #[error("HTTP client error: {0}")]
    HttpError(String),

    #[error("JSON parse error: {0}")]
    JsonError(String),
}

impl TranscriberError {
    pub fn message(&self, lang: &str) -> String {
        match self {
            Self::EmptyAudio => {
                if lang == "zh" {
                    "音频数据为空，请重新录音。".to_string()
                } else {
                    "Empty audio data. Please record again.".to_string()
                }
            }
            Self::MissingApiKey => {
                if lang == "zh" {
                    "转写 API 密钥未配置。请在设置中添加 API 密钥。".to_string()
                } else {
                    "Transcriber API key not configured. Add it in Settings.".to_string()
                }
            }
            Self::ApiError { status, body } => {
                if lang == "zh" {
                    format!("Whisper API 错误（HTTP {}）: {}", status, body)
                } else {
                    format!("Whisper API error (HTTP {}): {}", status, body)
                }
            }
            Self::WhisperCliNotFound { path } => {
                if lang == "zh" {
                    format!("whisper-cli 未找到: {}\n请在配置中设置正确的 whisper_path 或将 whisper-cli 添加到 PATH。", path)
                } else {
                    format!("whisper-cli not found: {}\nSet whisper_path in config or add whisper-cli to PATH.", path)
                }
            }
            Self::WhisperCliFailed { code, output } => {
                if lang == "zh" {
                    format!("whisper-cli 执行失败（退出码 {}）:\n{}", code, output)
                } else {
                    format!("whisper-cli failed (exit code {}):\n{}", code, output)
                }
            }
            Self::HttpError(msg) => {
                if lang == "zh" {
                    format!("HTTP 请求失败: {}", msg)
                } else {
                    format!("HTTP request failed: {}", msg)
                }
            }
            Self::JsonError(msg) => {
                if lang == "zh" {
                    format!("JSON 解析失败: {}", msg)
                } else {
                    format!("JSON parse error: {}", msg)
                }
            }
        }
    }
}

/// Polisher-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum PolisherError {
    #[error("Unknown protocol: {protocol}")]
    UnknownProtocol { protocol: String },

    #[error("API key not configured")]
    MissingApiKey,

    #[error("Rate limited")]
    RateLimited,

    #[error("LLM API returned {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("LLM returned empty response")]
    EmptyResponse,

    #[error("HTTP client error: {0}")]
    HttpError(String),

    #[error("JSON parse error: {0}")]
    JsonError(String),

    #[error("All retry attempts exhausted")]
    RetriesExhausted,
}

impl PolisherError {
    pub fn message(&self, lang: &str) -> String {
        match self {
            Self::UnknownProtocol { protocol } => {
                if lang == "zh" {
                    format!(
                        "未知的润色协议: '{}'。请使用 'openai' 或 'anthropic'。",
                        protocol
                    )
                } else {
                    format!(
                        "Unknown polisher protocol: '{}'. Use 'openai' or 'anthropic'.",
                        protocol
                    )
                }
            }
            Self::MissingApiKey => {
                if lang == "zh" {
                    "润色 API 密钥未配置。请在设置中添加 API 密钥。".to_string()
                } else {
                    "Polisher API key not configured. Add it in Settings.".to_string()
                }
            }
            Self::RateLimited => {
                if lang == "zh" {
                    "API 请求频率受限，请稍后重试。".to_string()
                } else {
                    "API rate limited. Please try again later.".to_string()
                }
            }
            Self::ApiError { status, body } => {
                if lang == "zh" {
                    format!("LLM API 错误（HTTP {}）: {}", status, body)
                } else {
                    format!("LLM API error (HTTP {}): {}", status, body)
                }
            }
            Self::EmptyResponse => {
                if lang == "zh" {
                    "LLM 返回空响应。".to_string()
                } else {
                    "LLM returned empty response.".to_string()
                }
            }
            Self::HttpError(msg) => {
                if lang == "zh" {
                    format!("HTTP 请求失败: {}", msg)
                } else {
                    format!("HTTP request failed: {}", msg)
                }
            }
            Self::JsonError(msg) => {
                if lang == "zh" {
                    format!("JSON 解析失败: {}", msg)
                } else {
                    format!("JSON parse error: {}", msg)
                }
            }
            Self::RetriesExhausted => {
                if lang == "zh" {
                    "所有重试尝试均失败。".to_string()
                } else {
                    "All retry attempts exhausted.".to_string()
                }
            }
        }
    }
}

/// Recorder-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    #[error("Failed to start recording: {0}")]
    StartFailed(String),

    #[error("Failed to stop recording: {0}")]
    StopFailed(String),

    #[error("Audio capture error: {0}")]
    CaptureFailed(String),

    #[error("Empty recording")]
    EmptyRecording,
}

impl RecorderError {
    pub fn message(&self, lang: &str) -> String {
        match self {
            Self::StartFailed(msg) => {
                if lang == "zh" {
                    format!("启动录音失败: {}", msg)
                } else {
                    format!("Failed to start recording: {}", msg)
                }
            }
            Self::StopFailed(msg) => {
                if lang == "zh" {
                    format!("停止录音失败: {}", msg)
                } else {
                    format!("Failed to stop recording: {}", msg)
                }
            }
            Self::CaptureFailed(msg) => {
                if lang == "zh" {
                    format!("音频捕获错误: {}", msg)
                } else {
                    format!("Audio capture error: {}", msg)
                }
            }
            Self::EmptyRecording => {
                if lang == "zh" {
                    "录音为空，请重试。".to_string()
                } else {
                    "Empty recording. Please try again.".to_string()
                }
            }
        }
    }
}

// Conversion from anyhow::Error for gradual migration was removed: modules now
// return typed errors directly (TranscriberError, PolisherError, RecorderError)
// and the pipeline aggregates them at the boundary.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fatal_error_classification() {
        let err = PipelineError::Fatal(FatalError::ModelNotFound {
            model: "base".to_string(),
            searched: vec![],
        });
        assert!(err.is_fatal());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_recoverable_error_classification() {
        let err = PipelineError::Recoverable(RecoverableError::EmptyTranscription);
        assert!(!err.is_fatal());
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_bilingual_messages_zh() {
        let err = PipelineError::Fatal(FatalError::ModelNotFound {
            model: "base".to_string(),
            searched: vec![PathBuf::from("/models")],
        });
        let msg = err.message("zh");
        assert!(msg.contains("本地模型未找到"));
        assert!(msg.contains("base"));
    }

    #[test]
    fn test_bilingual_messages_en() {
        let err = PipelineError::Fatal(FatalError::ModelNotFound {
            model: "base".to_string(),
            searched: vec![PathBuf::from("/models")],
        });
        let msg = err.message("en");
        assert!(msg.contains("Model 'base' not found"));
    }

    #[test]
    fn test_transcriber_error_messages() {
        let err = TranscriberError::MissingApiKey;
        assert!(err.message("zh").contains("API 密钥未配置"));
        assert!(err.message("en").contains("API key not configured"));
    }

    #[test]
    fn test_polisher_error_messages() {
        let err = PolisherError::RateLimited;
        assert!(err.message("zh").contains("频率受限"));
        assert!(err.message("en").contains("rate limited"));
    }
}
