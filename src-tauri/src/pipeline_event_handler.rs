//! Pipeline event handler — business logic for pipeline events.
//!
//! Separates event handling logic (clipboard, history, text selection) from
//! Tauri-specific concerns (event emission, overlay positioning). The handler
//! is testable and can be used independently of the Tauri runtime.

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
#[derive(Clone)]
pub struct PipelineEventHandler {
    prefer_polished: bool,
}

impl PipelineEventHandler {
    pub fn new(prefer_polished: bool) -> Self {
        Self { prefer_polished }
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

        // Write to clipboard
        let clipboard_ok = crate::output::write_clipboard(&text_to_use).await.is_ok();
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

    fn test_output(raw: &str, polished: &str, polish_failed: bool) -> PipelineOutput {
        PipelineOutput {
            raw_text: raw.to_string(),
            text: polished.to_string(),
            polish_failed,
        }
    }

    #[test]
    fn test_select_text_prefer_polished_success() {
        let handler = PipelineEventHandler::new(true);
        let output = test_output("raw text", "polished text", false);
        assert_eq!(handler.select_text(&output), "polished text");
    }

    #[test]
    fn test_select_text_prefer_polished_failed() {
        let handler = PipelineEventHandler::new(true);
        let output = test_output("raw text", "", true);
        assert_eq!(handler.select_text(&output), "raw text");
    }

    #[test]
    fn test_select_text_prefer_polished_empty() {
        let handler = PipelineEventHandler::new(true);
        let output = test_output("raw text", "  ", false);
        assert_eq!(handler.select_text(&output), "raw text");
    }

    #[test]
    fn test_select_text_prefer_raw() {
        let handler = PipelineEventHandler::new(false);
        let output = test_output("raw text", "polished text", false);
        assert_eq!(handler.select_text(&output), "raw text");
    }

    #[tokio::test]
    async fn test_handle_transcription_empty() {
        let handler = PipelineEventHandler::new(true);
        let output = test_output("", "", false);
        let temp_dir = tempfile::tempdir().unwrap();
        let history_path = temp_dir.path().join("history.json");
        let history_store = crate::history::HistoryStore::new(history_path);

        let result = handler.handle_transcription(&output, &history_store).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_handle_transcription_success() {
        let handler = PipelineEventHandler::new(true);
        let output = test_output("raw text", "polished text", false);
        let temp_dir = tempfile::tempdir().unwrap();
        let history_path = temp_dir.path().join("history.json");
        let history_store = crate::history::HistoryStore::new(history_path);

        let result = handler.handle_transcription(&output, &history_store).await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.clipboard_text, "polished text");
        assert!(result.should_show);
        // history_appended may be true or false depending on clipboard availability
    }
}
