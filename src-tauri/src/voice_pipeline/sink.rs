//! Pipeline event sink interface and shared types.
//! 管道事件接收器接口与共享类型。

use crate::pipeline_controller::PipelineStatus;

/// 转写结果。
/// Transcription result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptionResult {
    /// 处理后的文本（润色成功时为润色文本，否则为原始转写文本）
    /// Final text (polished text when polishing succeeded; raw transcription otherwise)
    pub text: String,
    /// 原始转写文本（润色前）
    /// Raw transcription (before polishing)
    pub raw_text: String,
    /// 润色是否失败
    /// Whether polishing failed
    pub polish_failed: bool,
    /// 润色失败时的错误信息（用于前端提示）
    /// Polish error message (for frontend display)
    #[serde(default)]
    pub polish_error: Option<String>,
}

/// 管道事件接收器。
///
/// 所有方法均为同步——实现方内部处理异步操作（如 `tokio::spawn`）。
/// 实现方必须是 `Send + Sync + 'static`，以支持跨线程使用。
///
/// Pipeline event sink.
///
/// All methods are synchronous—implementations handle async operations internally (e.g.
/// `tokio::spawn`). Implementations must be `Send + Sync + 'static` for cross-thread use.
pub trait PipelineSink: Send + Sync + 'static {
    /// 管道状态变化（idle / recording / processing / done / stopped）。
    /// Pipeline state change (idle / recording / processing / done / stopped).
    fn on_status_change(&self, status: PipelineStatus);

    /// 管道错误。
    /// Pipeline error.
    fn on_error(&self, message: &str);

    /// 转写+润色完成，输出结果。
    /// Transcription plus polishing finished; emit the result.
    fn on_transcription_result(&self, output: &TranscriptionResult);

    /// 转写/润色进度更新。`phase` 为 `"transcribe"` / `"polish"` / `"done"`，
    /// `fraction` 为 0–1 或 `None`（不确定进度）。
    /// Transcription/polishing progress update. `phase` is `"transcribe"` / `"polish"` /
    /// `"done"`; `fraction` is 0–1 or `None` (indeterminate progress).
    fn on_progress(&self, phase: &str, fraction: Option<f32>);

    /// 按键监听后端已启动（如 `"xinput"` / `"evtest"`）。
    /// The key listener backend has started (e.g. `"xinput"` / `"evtest"`).
    fn on_key_listener_backend(&self, backend: &str);

    /// 实时感知音频电平更新（0.0 ~ 1.0）。
    /// Realtime audio level updates (0.0 ~ 1.0).
    fn on_audio_level(&self, _level: f32) {}
}

/// 派发结果。
/// Dispatch outcome.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    /// 已写入剪贴板、待展示的文本。
    /// Text that was written to clipboard and should be shown.
    pub text: String,
    /// 历史是否追加成功。
    /// Whether history was appended successfully.
    pub history_appended: bool,
}
