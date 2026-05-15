//! 音频处理管道模块。
//!
//! 提供共享的语音处理核心逻辑：转写 + 润色。
//! 调用方负责处理输出（剪贴板写入、通知、GUI 状态更新）。

use anyhow::Context;

use crate::polisher::{LLMFormatter, PolishLevel};
use crate::transcriber::Transcriber;

/// 管道处理结果。
#[derive(Debug)]
pub struct PipelineOutput {
    /// 处理后的文本（润色成功时为润色文本，否则为原始转写文本）
    pub text: String,
    /// 原始转写文本（润色前）
    pub raw_text: String,
    /// 润色是否失败
    pub polish_failed: bool,
}

/// 运行核心管道：转写 → 润色。
///
/// 不包含剪贴板写入和通知 — 由调用方处理。
/// `on_progress`：`phase` 为 `"transcribe"` / `"polish"` / `"done"`，`fraction` 为 0–1 或 `None`（不确定进度）。
pub async fn process_audio_core(
    transcriber: &Transcriber,
    formatter: &LLMFormatter,
    wav_data: &[u8],
    polish_level: PolishLevel,
    mut on_progress: impl FnMut(&'static str, Option<f32>) + Send,
) -> anyhow::Result<PipelineOutput> {
    on_progress("transcribe", None);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
    let wav_owned = wav_data.to_vec();
    let transcribe_jh = tokio::spawn({
        let t = transcriber.clone();
        async move { t.transcribe(&wav_owned, Some(progress_tx)).await }
    });

    while let Some(fr) = progress_rx.recv().await {
        on_progress("transcribe", Some(fr));
    }

    let result = transcribe_jh
        .await
        .context("transcribe task join")?
        .context("transcribe")?;

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        on_progress("done", Some(1.0));
        return Ok(PipelineOutput {
            text: String::new(),
            raw_text: String::new(),
            polish_failed: false,
        });
    }

    on_progress("polish", None);

    // Step 2: Polish — preserve raw text for the floating window.
    let mut polish_failed = false;
    let raw_text = result.text.clone();
    let polished = formatter
        .polish(&raw_text, polish_level)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "polish failed, using raw text");
            polish_failed = true;
            raw_text.clone()
        });

    tracing::info!(text = %polished, "polished");

    on_progress("done", Some(1.0));

    Ok(PipelineOutput {
        text: polished,
        raw_text,
        polish_failed,
    })
}
