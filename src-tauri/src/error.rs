//! Structured error types for the altgo pipeline.
//!
//! Provides user-facing error messages (Chinese) and distinguishes
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

    /// Returns a user-facing error message in Chinese.
    pub fn message(&self) -> String {
        match self {
            Self::Fatal(e) => e.message(),
            Self::Recoverable(e) => e.message(),
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
}

impl FatalError {
    pub fn message(&self) -> String {
        match self {
            Self::ModelNotFound { model, searched } => {
                format!(
                    "本地模型未找到（配置值: {:?}）。\n搜索路径: {:?}\n请在 GUI 设置中下载模型，或将 [transcriber] model 设为已下载模型的名称（如 \"base\"）或完整文件路径。",
                    model, searched
                )
            }
            Self::ApiAuthFailed { service, status } => {
                format!(
                    "{} API 认证失败（HTTP {}）。请检查 API 密钥配置。",
                    service, status
                )
            }
            Self::KeyListenerFailed { backend, reason } => {
                format!("按键监听器启动失败（{}）: {}", backend, reason)
            }
            Self::TranscriberInitFailed(e) => e.message(),
            Self::PolisherInitFailed(e) => e.message(),
            Self::RecorderInitFailed(e) => e.message(),
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
}

impl RecoverableError {
    pub fn message(&self) -> String {
        match self {
            Self::TranscriptionFailed(e) => e.message(),
            Self::PolishingFailed(e) => e.message(),
            Self::RecordingFailed(e) => e.message(),
            Self::EmptyTranscription => "转写结果为空，请重试。".to_string(),
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
    pub fn message(&self) -> String {
        match self {
            Self::EmptyAudio => "音频数据为空，请重新录音。".to_string(),
            Self::MissingApiKey => "转写 API 密钥未配置。请在设置中添加 API 密钥。".to_string(),
            Self::ApiError { status, body } => {
                format!("Whisper API 错误（HTTP {}）: {}", status, body)
            }
            Self::WhisperCliNotFound { path } => {
                format!("whisper-cli 未找到: {}\n请在配置中设置正确的 whisper_path 或将 whisper-cli 添加到 PATH。", path)
            }
            Self::WhisperCliFailed { code, output } => {
                format!("whisper-cli 执行失败（退出码 {}）:\n{}", code, output)
            }
            Self::HttpError(msg) => format!("HTTP 请求失败: {}", msg),
            Self::JsonError(msg) => format!("JSON 解析失败: {}", msg),
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
    pub fn message(&self) -> String {
        match self {
            Self::UnknownProtocol { protocol } => {
                format!(
                    "未知的润色协议: '{}'。请使用 'openai' 或 'anthropic'。",
                    protocol
                )
            }
            Self::MissingApiKey => "润色 API 密钥未配置。请在设置中添加 API 密钥。".to_string(),
            Self::RateLimited => "API 请求频率受限，请稍后重试。".to_string(),
            Self::ApiError { status, body } => {
                format!("LLM API 错误（HTTP {}）: {}", status, body)
            }
            Self::EmptyResponse => "LLM 返回空响应。".to_string(),
            Self::HttpError(msg) => format!("HTTP 请求失败: {}", msg),
            Self::JsonError(msg) => format!("JSON 解析失败: {}", msg),
            Self::RetriesExhausted => "所有重试尝试均失败。".to_string(),
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
    pub fn message(&self) -> String {
        match self {
            Self::StartFailed(msg) => format!("启动录音失败: {}", msg),
            Self::StopFailed(msg) => format!("停止录音失败: {}", msg),
            Self::CaptureFailed(msg) => format!("音频捕获错误: {}", msg),
            Self::EmptyRecording => "录音为空，请重试。".to_string(),
        }
    }
}

// Conversion from anyhow::Error for gradual migration was removed: modules now
// return typed errors directly (TranscriberError, PolisherError, RecorderError)
// and the pipeline aggregates them at the boundary.

/// Output (clipboard) errors.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("no clipboard tool found")]
    NoClipboardTool,

    #[error("clipboard error: {0}")]
    ClipboardFailed(String),
}

/// Key listener errors.
#[derive(Debug, thiserror::Error)]
pub enum KeyListenerError {
    #[error("key listener tool not found: {0}")]
    ToolNotFound(String),

    #[error("unsupported activation key: '{0}'")]
    UnsupportedKey(String),

    #[error("key listener start failed: {0}")]
    StartFailed(String),

    #[error("keycode resolution failed: {0}")]
    ResolveFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Model management errors.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("unknown model: {0}")]
    UnknownModel(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("download failed: {0}")]
    DownloadFailed(String),

    #[error("HTTP error: {0}")]
    HttpError(String),
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    ParseError(String),

    #[error("TOML serialize error: {0}")]
    SerializeError(String),

    #[error("validation failed:\n{0}")]
    ValidationFailed(String),
}

/// History store errors.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(String),

    #[error("history entry not found: {0}")]
    NotFound(String),

    #[error("history lock poisoned")]
    LockPoisoned,

    #[error("serialization error: {0}")]
    SerializeError(String),
}

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
    fn test_fatal_error_messages() {
        let err = PipelineError::Fatal(FatalError::ModelNotFound {
            model: "base".to_string(),
            searched: vec![PathBuf::from("/models")],
        });
        let msg = err.message();
        assert!(msg.contains("本地模型未找到"));
        assert!(msg.contains("base"));
    }

    #[test]
    fn test_transcriber_error_messages() {
        let err = TranscriberError::MissingApiKey;
        assert!(err.message().contains("API 密钥未配置"));
    }

    #[test]
    fn test_polisher_error_messages() {
        let err = PolisherError::RateLimited;
        assert!(err.message().contains("频率受限"));
    }
}
