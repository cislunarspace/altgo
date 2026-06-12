//! Pipeline command handlers — testable business logic for state machine commands.
//!
//! Separates command handling logic from the event loop, making it testable
//! without running the full pipeline orchestrator.

use crate::pipeline_sink::PipelineSink;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::{PlatformRecorder, Recorder};
use crate::transcriber::Transcriber;

/// Handle StartRecord command: start recording and notify sink.
pub fn handle_start_record(
    recorder: &mut PlatformRecorder,
    sink: &impl PipelineSink,
) -> Result<(), String> {
    tracing::info!("recording started");
    recorder.start_recording().map_err(|e: anyhow::Error| {
        tracing::error!(error = %e, "failed to start recording");
        e.to_string()
    })?;
    sink.on_status_change("recording");
    Ok(())
}

/// Handle StopRecord command: stop recording, process audio, notify sink.
pub async fn handle_stop_record(
    recorder: &mut PlatformRecorder,
    transcriber: &Transcriber,
    formatter: &LLMFormatter,
    polish_level: PolishLevel,
    sink: &impl PipelineSink,
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

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
    let wav_owned = wav_data.to_vec();
    let transcribe_jh = tokio::spawn({
        let t = transcriber.clone();
        async move { t.transcribe(&wav_owned, Some(progress_tx)).await }
    });

    while let Some(fr) = progress_rx.recv().await {
        sink.on_progress("transcribe", Some(fr));
    }

    let result = match transcribe_jh.await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            tracing::error!(error = %e, "transcription failed");
            sink.on_error(&format!("transcription: {}", e));
            return;
        }
        Err(e) => {
            tracing::error!(error = %e, "transcribe task join");
            sink.on_error("transcription: task join");
            return;
        }
    };

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        sink.on_progress("done", Some(1.0));
        sink.on_transcription_result(&crate::pipeline::PipelineOutput {
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

    let output = crate::pipeline::PipelineOutput {
        text: polished,
        raw_text,
        polish_failed,
    };
    sink.on_transcription_result(&output);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::PipelineOutput;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockSink {
        status_changes: Arc<Mutex<Vec<String>>>,
        errors: Arc<Mutex<Vec<String>>>,
        results: Arc<Mutex<Vec<PipelineOutput>>>,
    }

    impl MockSink {
        fn new() -> Self {
            Self {
                status_changes: Arc::new(Mutex::new(Vec::new())),
                errors: Arc::new(Mutex::new(Vec::new())),
                results: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn status_changes(&self) -> Vec<String> {
            self.status_changes.lock().unwrap().clone()
        }

        fn errors(&self) -> Vec<String> {
            self.errors.lock().unwrap().clone()
        }
    }

    impl PipelineSink for MockSink {
        fn on_status_change(&self, status: &str) {
            self.status_changes.lock().unwrap().push(status.to_string());
        }

        fn on_error(&self, message: &str) {
            self.errors.lock().unwrap().push(message.to_string());
        }

        fn on_transcription_result(&self, output: &PipelineOutput) {
            self.results.lock().unwrap().push(output.clone());
        }

        fn on_progress(&self, _phase: &str, _fraction: Option<f32>) {}

        fn on_key_listener_backend(&self, _backend: &str) {}
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_handle_start_record_success() {
        let mut recorder = PlatformRecorder::new(16000, 1);
        let sink = MockSink::new();

        let result = handle_start_record(&mut recorder, &sink);
        assert!(result.is_ok());
        assert_eq!(sink.status_changes(), vec!["recording"]);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_handle_start_record_returns_error_on_windows_stub() {
        let mut recorder = PlatformRecorder::new(16000, 1);
        let sink = MockSink::new();

        let result = handle_start_record(&mut recorder, &sink);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not implemented yet"));
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_handle_stop_record_empty_audio() {
        let mut recorder = PlatformRecorder::new(16000, 1);
        let sink = MockSink::new();

        // Start and immediately stop to get empty audio
        let _ = recorder.start_recording();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let transcriber =
            crate::transcriber::Transcriber::Local(crate::transcriber::LocalWhisper::new(
                "/nonexistent/model".to_string(),
                "zh".to_string(),
                "whisper-cli".to_string(),
                0,
                0,
            ));
        let formatter = crate::polisher::LLMFormatter::new(
            "test-key".to_string(),
            "http://localhost".to_string(),
            "test-model".to_string(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();

        handle_stop_record(
            &mut recorder,
            &transcriber,
            &formatter,
            PolishLevel::None,
            &sink,
        )
        .await;

        let statuses = sink.status_changes();
        assert!(statuses.contains(&"processing".to_string()));
        // Will either go to idle (on stop error) or error (on transcription failure)
        assert!(
            statuses.contains(&"idle".to_string()) || !sink.errors().is_empty(),
            "Expected idle status or error, got statuses: {:?}, errors: {:?}",
            statuses,
            sink.errors()
        );
    }
}
