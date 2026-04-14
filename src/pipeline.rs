//! 音频处理管道模块。
//!
//! 提供共享的语音处理核心逻辑：转写 + 润色。
//! 调用方负责处理输出（剪贴板写入、通知、GUI 状态更新）。

use crate::polisher::{LLMFormatter, PolishLevel};
use crate::transcriber::Transcriber;

/// 管道处理结果。
#[derive(Debug)]
pub struct PipelineOutput {
    /// 处理后的文本（润色成功时为润色文本，否则为原始转写文本）
    pub text: String,
    /// 润色是否失败
    pub polish_failed: bool,
}

/// 运行核心管道：转写 → 润色。
///
/// 不包含剪贴板写入和通知 — 由调用方处理。
pub(crate) async fn process_audio_core(
    transcriber: &Transcriber,
    formatter: &LLMFormatter,
    wav_data: &[u8],
    polish_level: PolishLevel,
) -> anyhow::Result<PipelineOutput> {
    // Step 1: Transcribe.
    let result = transcriber.transcribe(wav_data).await?;

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        return Ok(PipelineOutput {
            text: String::new(),
            polish_failed: false,
        });
    }

    // Step 2: Polish.
    let mut polish_failed = false;
    let polished = formatter
        .polish(&result.text, polish_level)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "polish failed, using raw text");
            polish_failed = true;
            result.text.clone()
        });

    tracing::info!(text = %polished, "polished");

    Ok(PipelineOutput {
        text: polished,
        polish_failed,
    })
}
