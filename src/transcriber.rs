use anyhow::{anyhow, Context};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

/// Result of a transcription.
#[derive(Debug)]
pub struct TranscribeResult {
    pub text: String,
    pub language: String,
}

#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
    language: Option<String>,
}

/// OpenAI Whisper API transcriber.
pub struct WhisperApi {
    api_key: String,
    api_base_url: String,
    model: String,
    language: String,
    client: Client,
}

impl WhisperApi {
    pub fn new(
        api_key: String,
        api_base_url: String,
        model: String,
        language: String,
        timeout: Duration,
    ) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            api_key,
            api_base_url,
            model,
            language,
            client,
        }
    }

    pub async fn transcribe(&self, audio_data: &[u8]) -> anyhow::Result<TranscribeResult> {
        if audio_data.is_empty() {
            return Err(anyhow!("empty audio data"));
        }
        if self.api_key.is_empty() {
            return Err(anyhow!("transcriber API key not configured"));
        }

        let url = format!("{}/v1/audio/transcriptions", self.api_base_url);

        let audio_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let form = reqwest::multipart::Form::new()
            .part("file", audio_part)
            .text("model", self.model.clone())
            .text("language", self.language.clone());

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .context("Whisper API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Whisper API returned {}: {}", status, body));
        }

        let result: WhisperResponse = resp
            .json()
            .await
            .context("failed to parse Whisper response")?;

        Ok(TranscribeResult {
            text: result.text,
            language: result.language.unwrap_or_default(),
        })
    }
}

/// Local whisper.cpp transcriber using FFI.
///
/// This requires whisper.cpp shared libraries to be available at runtime.
pub struct LocalWhisper {
    model_path: String,
    language: String,
}

impl LocalWhisper {
    pub fn new(model_path: String, language: String) -> Self {
        Self {
            model_path,
            language,
        }
    }

    /// Transcribe using the `whisper-cpp` binary as a subprocess.
    ///
    /// This is a simpler approach than FFI and avoids build complexity.
    /// The whisper.cpp `main` binary must be built and available in PATH or
    /// at `whisper.cpp/build/bin/whisper-cli`.
    pub async fn transcribe(&self, audio_data: &[u8]) -> anyhow::Result<TranscribeResult> {
        if audio_data.is_empty() {
            return Err(anyhow!("empty audio data"));
        }

        // Write audio to a temp file.
        let tmp_dir = tempfile::tempdir().context("create temp dir")?;
        let wav_path = tmp_dir.path().join("audio.wav");
        std::fs::write(&wav_path, audio_data).context("write temp wav file")?;

        // Find whisper-cli binary.
        let whisper_bin = self.find_whisper_binary()?;

        let mut cmd = tokio::process::Command::new(&whisper_bin);
        cmd.arg("-m")
            .arg(&self.model_path)
            .arg("-l")
            .arg(&self.language)
            .arg("-f")
            .arg(&wav_path)
            .arg("--no-timestamps")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let output = cmd.output().await.context("failed to run whisper-cli")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("whisper-cli failed: {}", stderr));
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(TranscribeResult {
            text,
            language: self.language.clone(),
        })
    }

    fn find_whisper_binary(&self) -> anyhow::Result<String> {
        // Check common locations.
        let candidates = [
            "whisper-cli",
            "whisper-cpp",
            "./whisper.cpp/build/bin/whisper-cli",
            "./whisper.cpp/bin/whisper-cli",
            &format!(
                "{}/whisper.cpp/build/bin/whisper-cli",
                std::env::current_dir()?.display()
            ),
        ];

        for candidate in &candidates {
            if std::path::Path::new(candidate).exists() || which_exists(candidate) {
                return Ok(candidate.to_string());
            }
        }

        Err(anyhow!(
            "whisper-cli not found — build whisper.cpp and add to PATH"
        ))
    }
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transcribe_empty_audio() {
        let api = WhisperApi::new(
            "key".to_string(),
            "http://localhost".to_string(),
            "whisper-1".to_string(),
            "zh".to_string(),
            Duration::from_secs(5),
        );
        let result = api.transcribe(&[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_transcribe_no_api_key() {
        let api = WhisperApi::new(
            String::new(),
            "http://localhost".to_string(),
            "whisper-1".to_string(),
            "zh".to_string(),
            Duration::from_secs(5),
        );
        let result = api.transcribe(&[0u8; 44]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key"));
    }

    #[tokio::test]
    async fn test_transcribe_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/audio/transcriptions")
            .match_header("Authorization", "Bearer test-key")
            .with_status(200)
            .with_body(
                serde_json::json!({
                    "text": "你好世界",
                    "language": "zh"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let api = WhisperApi::new(
            "test-key".to_string(),
            server.url(),
            "whisper-1".to_string(),
            "zh".to_string(),
            Duration::from_secs(5),
        );
        let result = api.transcribe(&[0u8; 44]).await.unwrap();
        assert_eq!(result.text, "你好世界");
        assert_eq!(result.language, "zh");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_transcribe_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/audio/transcriptions")
            .with_status(401)
            .with_body("unauthorized")
            .create_async()
            .await;

        let api = WhisperApi::new(
            "bad-key".to_string(),
            server.url(),
            "whisper-1".to_string(),
            "zh".to_string(),
            Duration::from_secs(5),
        );
        let result = api.transcribe(&[0u8; 44]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_local_whisper_empty_audio() {
        let lw = LocalWhisper::new("/path/to/model".to_string(), "zh".to_string());
        let result = lw.transcribe(&[]).await;
        assert!(result.is_err());
    }
}
