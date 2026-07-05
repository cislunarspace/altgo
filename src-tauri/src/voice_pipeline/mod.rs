//! Voice Pipeline — 拥有完整语音转文字管道。
//!
//! 模块划分：
//! - `sink` — 共享类型（PipelineOutput、PipelineSink、ProcessedResult）
//! - `builder` — PipelineBuilder 组件构造
//! - `context` — PipelineContext 事件循环
//! - `handlers` — 命令处理器与结果处理
//!
//! 公共接口：
//! - `run(cfg, stop_rx, sink)` — 入口（构建 context + 运行事件循环）
//! - `PipelineBuilder` — 单独构造各组件（可测试）
//! - `PipelineContext` — 拥有组件，暴露 `run(stop_rx, sink)`

mod builder;
mod context;
mod handlers;
mod sink;

#[cfg(test)]
mod test_doubles;

pub use builder::PipelineBuilder;
pub use context::PipelineContext;
pub use handlers::{
    handle_start_record, handle_stop_record, process_transcription_result, select_text,
};
pub use sink::{PipelineOutput, PipelineSink, ProcessedResult};

use std::sync::Arc;

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