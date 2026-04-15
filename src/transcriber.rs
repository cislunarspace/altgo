//! 语音识别模块。
//!
//! 提供两种语音识别后端：
//!
//! - `WhisperApi`：通过 HTTP multipart 请求调用兼容 OpenAI 的 Whisper API
//! - `LocalWhisper`：通过子进程调用本地 `whisper-cli` 二进制文件
//!
//! 两种后端均返回 `TranscribeResult`，包含识别文本和语言信息。

use anyhow::{anyhow, Context};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

/// 语音识别结果。
#[derive(Debug)]
#[allow(dead_code)]
pub struct TranscribeResult {
    /// 识别出的文本
    pub text: String,
    /// 检测到的语言代码
    pub language: String,
}

#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
    language: Option<String>,
}

/// OpenAI Whisper API 语音识别器。
///
/// 通过 HTTP multipart 请求调用兼容 OpenAI 的 Whisper API 端点。
#[derive(Clone)]
pub struct WhisperApi {
    api_key: String,
    api_base_url: String,
    model: String,
    language: String,
    temperature: f32,
    prompt: String,
    client: Client,
}

impl WhisperApi {
    pub fn new(
        api_key: String,
        api_base_url: String,
        model: String,
        language: String,
        temperature: f32,
        prompt: String,
        timeout: Duration,
    ) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .context("failed to build HTTP client for WhisperApi")?;
        Ok(Self {
            api_key,
            api_base_url,
            model,
            language,
            temperature,
            prompt,
            client,
        })
    }

    /// 通过 API 识别音频数据，返回识别结果。
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

        let mut form = reqwest::multipart::Form::new()
            .part("file", audio_part)
            .text("model", self.model.clone())
            .text("language", self.language.clone())
            .text("temperature", format!("{}", self.temperature));

        if !self.prompt.is_empty() {
            form = form.text("prompt", self.prompt.clone());
        }

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
            let body = resp
                .text()
                .await
                .context("failed to read Whisper API error body")?;
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

/// 本地 whisper.cpp 语音识别器。
///
/// 通过子进程调用 `whisper-cli` 二进制文件，避免 FFI 构建复杂性。
#[derive(Clone)]
pub struct LocalWhisper {
    model_path: String,
    language: String,
    whisper_path: String,
}

impl LocalWhisper {
    /// 创建新的本地语音识别器。
    ///
    /// `model_path`：whisper 模型文件路径
    /// `language`：语言代码
    /// `whisper_path`：whisper-cli 二进制文件路径（为空时自动查找）
    pub fn new(model_path: String, language: String, whisper_path: String) -> Self {
        Self {
            model_path,
            language,
            whisper_path,
        }
    }

    /// 通过本地 `whisper-cli` 子进程识别音频数据。
    ///
    /// 音频数据先写入临时文件，然后调用 whisper-cli 进行识别。
    pub async fn transcribe(&self, audio_data: &[u8]) -> anyhow::Result<TranscribeResult> {
        if audio_data.is_empty() {
            return Err(anyhow!("empty audio data"));
        }

        // Write audio to a temp file.
        let tmp_dir = tempfile::tempdir().context("create temp dir")?;
        let wav_path = tmp_dir.path().join("audio.wav");
        std::fs::write(&wav_path, audio_data).context("write temp wav file")?;

        // Find whisper-cli binary.
        let whisper_bin = find_whisper_binary(&self.whisper_path)?;

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
}

/// 查找 whisper-cli 二进制文件。
///
/// 查找顺序：
/// 1. 用户通过配置指定的路径（`whisper_path`）
/// 2. 系统 PATH 中的 `whisper-cli` 和 `whisper-cpp`
fn find_whisper_binary(whisper_path: &str) -> anyhow::Result<std::path::PathBuf> {
    static CACHE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

    if let Some(cached) = CACHE.get() {
        return Ok(cached.clone());
    }

    // 1. Use explicitly configured path.
    if !whisper_path.is_empty() {
        let path = std::path::Path::new(whisper_path);
        if path.exists() {
            let buf = path.to_path_buf();
            let _ = CACHE.set(buf.clone());
            return Ok(buf);
        }
        return Err(anyhow!(
            "whisper-cli not found at configured path: {}",
            whisper_path
        ));
    }

    // 2. Search on PATH using `which` (Linux/macOS) or `where` (Windows).
    let candidates = ["whisper-cli", "whisper-cpp"];
    for candidate in &candidates {
        if let Ok(found) = which_binary(candidate) {
            let _ = CACHE.set(found.clone());
            return Ok(found);
        }
    }

    Err(anyhow!(
        "whisper-cli not found — set whisper_path in config or add whisper-cli to PATH"
    ))
}

/// Search for a binary on the system PATH.
#[cfg(unix)]
fn which_binary(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let output = std::process::Command::new("which").arg(name).output()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(anyhow!("{} not found on PATH", name))
}

/// Search for a binary on the system PATH.
#[cfg(windows)]
fn which_binary(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let output = std::process::Command::new("cmd")
        .args(["/C", "where", name])
        .output()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(anyhow!("{} not found on PATH", name))
}

/// Unified transcription backend — dispatches between API and local engines.
#[derive(Clone)]
pub enum Transcriber {
    Api(WhisperApi),
    Local(LocalWhisper),
}

impl Transcriber {
    /// Transcribe audio data using the selected backend.
    pub async fn transcribe(&self, wav_data: &[u8]) -> anyhow::Result<TranscribeResult> {
        match self {
            Transcriber::Api(api) => api.transcribe(wav_data).await,
            Transcriber::Local(lw) => lw.transcribe(wav_data).await,
        }
    }
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
            0.0,
            String::new(),
            Duration::from_secs(5),
        )
        .unwrap();
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
            0.0,
            String::new(),
            Duration::from_secs(5),
        )
        .unwrap();
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
            0.0,
            String::new(),
            Duration::from_secs(5),
        )
        .unwrap();
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
            0.0,
            String::new(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = api.transcribe(&[0u8; 44]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_local_whisper_empty_audio() {
        let lw = LocalWhisper::new(
            "/path/to/model".to_string(),
            "zh".to_string(),
            String::new(),
        );
        let result = lw.transcribe(&[]).await;
        assert!(result.is_err());
    }
}
