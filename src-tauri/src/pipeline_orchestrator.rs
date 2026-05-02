//! 管道编排器。
//!
//! 负责语音转文字管道的事件循环：按键事件 → 状态机 → 录音 → 转写 → 润色。
//! 通过 `PipelineSink` trait 报告状态和结果，不依赖 Tauri 或具体的输出方式。

use std::sync::Arc;

use crate::pipeline_sink::PipelineSink;

/// 运行语音管道。
///
/// 阻塞当前异步任务直到收到 `stop_rx` 信号。
/// 所有状态变化和处理结果通过 `sink` 报告。
pub async fn run(
    cfg: Arc<crate::config::Config>,
    stop_rx: tokio::sync::oneshot::Receiver<()>,
    sink: impl PipelineSink,
) {
    let builder = crate::pipeline_builder::PipelineBuilder::new(cfg.clone());

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
