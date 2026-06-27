//! 命令处理器与结果处理。
//!
//! `handle_start_record` / `handle_stop_record` 是按状态机命令调用的纯业务逻辑。
//! `process_transcription_result` 处理转写完成后的剪贴板写入和历史追加。

use std::sync::Arc;

use crate::history::HistoryStore;
use crate::output::Output;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::Recorder;
use crate::transcriber::Transcriber;

use super::sink::{PipelineOutput, PipelineSink, ProcessedResult};

/// Handle StartRecord command: start recording and notify sink.
pub fn handle_start_record(
    recorder: &mut dyn Recorder,
    sink: &(impl PipelineSink + ?Sized),
) -> Result<(), String> {
    tracing::info!("recording started");
    recorder
        .start_recording()
        .map_err(|e: crate::error::RecorderError| {
            tracing::error!(error = %e, "failed to start recording");
            e.to_string()
        })?;
    sink.on_status_change("recording");
    Ok(())
}

/// Handle StopRecord command: stop recording, process audio, notify sink.
pub async fn handle_stop_record(
    recorder: &mut dyn Recorder,
    transcriber: &dyn Transcriber,
    formatter: &LLMFormatter,
    polish_level: PolishLevel,
    sink: Arc<dyn PipelineSink>,
) {
    tracing::info!("recording stopped, processing...");
    sink.on_status_change("processing");

    let wav_data: Vec<u8> = match recorder.stop_recording() {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(error = %e, "failed to stop recording");
            sink.on_status_change("idle");
            return;
        }
    };

    sink.on_progress("transcribe", None);

    // Bridge the trait's Arc<dyn Fn> progress callback to the sink — the
    // callback must own its data, so we run a small forwarder task that
    // listens to an mpsc channel and invokes the callback.
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
    let progress_cb: Arc<dyn Fn(f32) + Send + Sync> = Arc::new(move |fr: f32| {
        let _ = progress_tx.send(fr);
    });
    let forwarder_sink = sink.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(fr) = progress_rx.recv().await {
            forwarder_sink.on_progress("transcribe", Some(fr));
        }
    });

    let transcribe_result = transcriber.transcribe(&wav_data, progress_cb).await;
    let _ = forwarder.await;
    let result = match transcribe_result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "transcription failed");
            sink.on_error(&format!("transcription: {}", e));
            sink.on_status_change("idle");
            return;
        }
    };

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        sink.on_progress("done", Some(1.0));
        sink.on_transcription_result(&PipelineOutput {
            text: String::new(),
            raw_text: String::new(),
            polish_failed: false,
        });
        return;
    }

    sink.on_progress("polish", None);

    let mut polish_failed = false;
    let raw_text = result.text.clone();
    let polished = match formatter.polish(&raw_text, polish_level).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "polish failed, using raw text");
            polish_failed = true;
            raw_text.clone()
        }
    };

    tracing::info!(text = %polished, "polished");

    sink.on_progress("done", Some(1.0));

    let output = PipelineOutput {
        text: polished,
        raw_text,
        polish_failed,
    };
    sink.on_transcription_result(&output);
}

/// Select which text to use based on preferences and polish status.
pub fn select_text(prefer_polished: bool, output: &PipelineOutput) -> String {
    if prefer_polished && !output.polish_failed && !output.text.trim().is_empty() {
        output.text.clone()
    } else {
        output.raw_text.clone()
    }
}

/// Process a transcription result: select text, write clipboard, append history.
///
/// Returns `None` if the transcription was empty (no action taken).
pub async fn process_transcription_result(
    output: &PipelineOutput,
    prefer_polished: bool,
    output_adapter: &dyn Output,
    history_store: &HistoryStore,
) -> Option<ProcessedResult> {
    if output.raw_text.is_empty() {
        return None;
    }

    let text_to_use = select_text(prefer_polished, output);

    // Write to clipboard (blocking I/O; caller is already in an async context)
    let text_clone = text_to_use.clone();
    let output_handle = output_adapter.clone_box();
    let clipboard_ok =
        tokio::task::spawn_blocking(move || output_handle.write_clipboard(&text_clone))
            .await
            .ok()
            .and_then(|r| r.ok())
            .is_some();
    if !clipboard_ok {
        tracing::warn!("failed to write clipboard");
    }

    // Append to history
    let raw = output.raw_text.clone();
    let display = text_to_use.clone();
    let store = history_store.clone();
    let history_appended = tokio::task::spawn_blocking(move || store.append(raw, display))
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some();

    if !history_appended {
        tracing::warn!("failed to append transcription history");
    }

    Some(ProcessedResult {
        text: text_to_use,
        history_appended,
    })
}
