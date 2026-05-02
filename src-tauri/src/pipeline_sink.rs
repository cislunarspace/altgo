//! 管道输出接收器 trait。
//!
//! `PipelineSink` 将管道事件与具体的输出方式（Tauri 事件、剪贴板、通知等）解耦。
//! 管道编排器通过此 trait 报告状态变化和处理结果，由调用方决定如何处理。

use crate::pipeline::PipelineOutput;

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
