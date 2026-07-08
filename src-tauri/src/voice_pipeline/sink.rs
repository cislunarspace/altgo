//! 管道事件接收器接口与共享类型。

use crate::pipeline_controller::PipelineStatus;

/// 转写结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptionResult {
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
    fn on_status_change(&self, status: PipelineStatus);

    /// 管道错误。
    fn on_error(&self, message: &str);

    /// 转写+润色完成，输出结果。
    fn on_transcription_result(&self, output: &TranscriptionResult);

    /// 转写/润色进度更新。`phase` 为 `"transcribe"` / `"polish"` / `"done"`，
    /// `fraction` 为 0–1 或 `None`（不确定进度）。
    fn on_progress(&self, phase: &str, fraction: Option<f32>);

    /// 按键监听后端已启动（如 `"xinput"` / `"evtest"`）。
    fn on_key_listener_backend(&self, backend: &str);
}

/// 派发结果。
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    /// Text that was written to clipboard and should be shown.
    pub text: String,
    /// Whether history was appended successfully.
    pub history_appended: bool,
}
