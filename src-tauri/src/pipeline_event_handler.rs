//! Pipeline event handler — business logic for pipeline events.
//!
//! Separates event handling logic (clipboard, history, text selection) from
//! Tauri-specific concerns (event emission, overlay positioning). The handler
//! is testable and can be used independently of the Tauri runtime.

use crate::output::Output;
use crate::pipeline::PipelineOutput;

/// Result of processing a transcription event.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// Text written to clipboard (empty if transcription was empty).
    pub clipboard_text: String,
    /// Whether history was appended successfully.
    pub history_appended: bool,
    /// Whether the result should be shown (false if empty transcription).
    pub should_show: bool,
}

/// Pipeline event handler — processes pipeline events with business logic.
///
/// Handles:
/// - Text selection (prefer polished vs raw)
/// - Clipboard writing
/// - History persistence
///
/// Does NOT handle:
/// - Tauri event emission
/// - Overlay window management
/// - Status updates
pub struct PipelineEventHandler {
    prefer_polished: bool,
    output: Box<dyn Output>,
}

impl Clone for PipelineEventHandler {
    fn clone(&self) -> Self {
        Self {
            prefer_polished: self.prefer_polished,
            output: self.output.clone_box(),
        }
    }
}

impl PipelineEventHandler {
    pub fn new(prefer_polished: bool, output: Box<dyn Output>) -> Self {
        Self {
            prefer_polished,
            output,
        }
    }

    /// Select which text to use based on preferences and polish status.
    pub fn select_text(&self, output: &PipelineOutput) -> String {
        if self.prefer_polished && !output.polish_failed && !output.text.trim().is_empty() {
            output.text.clone()
        } else {
            output.raw_text.clone()
        }
    }

    /// Process transcription result: select text, write clipboard, append history.
    ///
    /// Returns `None` if transcription was empty (no action taken).
    pub async fn handle_transcription(
        &self,
        output: &PipelineOutput,
        history_store: &crate::history::HistoryStore,
    ) -> Option<TranscriptionResult> {
        if output.raw_text.is_empty() {
            return None;
        }

        let text_to_use = self.select_text(output);

        // Write to clipboard (blocking I/O; caller is already in an async context)
        let text_clone = text_to_use.clone();
        let output_handle = self.output.clone_box();
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

        Some(TranscriptionResult {
            clipboard_text: text_to_use,
            history_appended,
            should_show: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FakeOutput {
        clipboard_writes: Arc<Mutex<Vec<String>>>,
    }

    impl FakeOutput {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    clipboard_writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl Output for FakeOutput {
        fn write_clipboard(&self, text: &str) -> anyhow::Result<()> {
            self.clipboard_writes.lock().unwrap().push(text.to_string());
            Ok(())
        }

        fn notify(&self, _title: &str, _body: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn clone_box(&self) -> Box<dyn Output> {
            Box::new(FakeOutput {
                clipboard_writes: Arc::clone(&self.clipboard_writes),
            })
        }
    }

    fn test_output(raw: &str, polished: &str, polish_failed: bool) -> PipelineOutput {
        PipelineOutput {
            raw_text: raw.to_string(),
            text: polished.to_string(),
            polish_failed,
        }
    }

    fn test_handler(prefer_polished: bool) -> (PipelineEventHandler, Arc<Mutex<Vec<String>>>) {
        let (output, writes) = FakeOutput::new();
        (
            PipelineEventHandler::new(prefer_polished, Box::new(output)),
            writes,
        )
    }

    #[test]
    fn test_select_text_prefer_polished_success() {
        let (handler, _) = test_handler(true);
        let output = test_output("raw text", "polished text", false);
        assert_eq!(handler.select_text(&output), "polished text");
    }

    #[test]
    fn test_select_text_prefer_polished_failed() {
        let (handler, _) = test_handler(true);
        let output = test_output("raw text", "", true);
        assert_eq!(handler.select_text(&output), "raw text");
    }

    #[test]
    fn test_select_text_prefer_polished_empty() {
        let (handler, _) = test_handler(true);
        let output = test_output("raw text", "  ", false);
        assert_eq!(handler.select_text(&output), "raw text");
    }

    #[test]
    fn test_select_text_prefer_raw() {
        let (handler, _) = test_handler(false);
        let output = test_output("raw text", "polished text", false);
        assert_eq!(handler.select_text(&output), "raw text");
    }

    #[tokio::test]
    async fn test_handle_transcription_empty() {
        let (handler, _) = test_handler(true);
        let output = test_output("", "", false);
        let temp_dir = tempfile::tempdir().unwrap();
        let history_path = temp_dir.path().join("history.json");
        let history_store = crate::history::HistoryStore::new(history_path);

        let result = handler.handle_transcription(&output, &history_store).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_handle_transcription_success() {
        let (handler, writes) = test_handler(true);
        let output = test_output("raw text", "polished text", false);
        let temp_dir = tempfile::tempdir().unwrap();
        let history_path = temp_dir.path().join("history.json");
        let history_store = crate::history::HistoryStore::new(history_path);

        let result = handler.handle_transcription(&output, &history_store).await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.clipboard_text, "polished text");
        assert!(result.should_show);
        assert_eq!(writes.lock().unwrap().len(), 1);
        assert_eq!(writes.lock().unwrap()[0], "polished text");
    }

    #[tokio::test]
    async fn test_handle_transcription_clipboard_failure_still_returns_result() {
        struct FailingOutput;
        impl Output for FailingOutput {
            fn write_clipboard(&self, _text: &str) -> anyhow::Result<()> {
                Err(anyhow::anyhow!("no clipboard"))
            }
            fn notify(&self, _: &str, _: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn clone_box(&self) -> Box<dyn Output> {
                Box::new(FailingOutput)
            }
        }

        let handler = PipelineEventHandler::new(true, Box::new(FailingOutput));
        let output = test_output("raw text", "polished text", false);
        let temp_dir = tempfile::tempdir().unwrap();
        let history_store = crate::history::HistoryStore::new(temp_dir.path().join("history.json"));

        let result = handler.handle_transcription(&output, &history_store).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().clipboard_text, "polished text");
    }
}
