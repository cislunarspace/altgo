//! Pipeline command handlers — testable business logic for state machine commands.
//!
//! Separates command handling logic from the event loop, making it testable
//! without running the full pipeline orchestrator.

use crate::pipeline_sink::PipelineSink;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::PlatformRecorder;
use crate::transcriber::Transcriber;

/// Handle StartRecord command: start recording and notify sink.
pub fn handle_start_record(
    recorder: &mut PlatformRecorder,
    sink: &impl PipelineSink,
) -> Result<(), String> {
    tracing::info!("recording started");
    recorder.start().map_err(|e| {
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

    let wav_data = match recorder.stop() {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(error = %e, "failed to stop recording");
            sink.on_status_change("idle");
            return;
        }
    };

    match crate::pipeline::process_audio_core(
        transcriber,
        formatter,
        &wav_data,
        polish_level,
        |phase, fraction| {
            sink.on_progress(phase, fraction);
        },
    )
    .await
    {
        Ok(output) => {
            sink.on_transcription_result(&output);
        }
        Err(e) => {
            tracing::error!(error = %e, "audio processing failed");
            sink.on_error(&format!("processing: {}", e));
        }
    }
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
    fn test_handle_start_record_success() {
        let mut recorder = PlatformRecorder::new(16000, 1);
        let sink = MockSink::new();

        let result = handle_start_record(&mut recorder, &sink);
        assert!(result.is_ok());
        assert_eq!(sink.status_changes(), vec!["recording"]);
    }

    #[tokio::test]
    async fn test_handle_stop_record_empty_audio() {
        let mut recorder = PlatformRecorder::new(16000, 1);
        let sink = MockSink::new();

        // Start and immediately stop to get empty audio
        let _ = recorder.start();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let transcriber = crate::transcriber::Transcriber::Local(
            crate::transcriber::LocalWhisper::new(
                "/nonexistent/model".to_string(),
                "zh".to_string(),
                "whisper-cli".to_string(),
            ),
        );
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
