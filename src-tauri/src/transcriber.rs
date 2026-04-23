//! 语音识别模块。
//!
//! 提供两种语音识别后端：
//!
//! - `WhisperApi`：通过 HTTP multipart 请求调用兼容 OpenAI 的 Whisper API
//! - `LocalWhisper`：通过子进程调用本地 `whisper-cli` 二进制文件
//!
//! 两种后端均返回 `TranscribeResult`，包含识别文本和语言信息。

use anyhow::{anyhow, Context};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;

/// Expand tilde in path to home directory.
fn stderr_fraction(line: &str, re_percent: &Regex, re_ratio: &Regex) -> Option<f32> {
    if let Some(c) = re_percent.captures(line) {
        if let Ok(p) = c[1].parse::<u32>() {
            if p <= 100 {
                return Some((p as f32 / 100.0).clamp(0.0, 1.0));
            }
        }
    }
    if let Some(c) = re_ratio.captures(line) {
        if let (Ok(a), Ok(b)) = (c[1].parse::<u64>(), c[2].parse::<u64>()) {
            if b > 0 {
                return Some(((a as f32) / (b as f32)).clamp(0.0, 1.0));
            }
        }
    }
    None
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(path)
}

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
    /// `progress` 在解析到 stderr 中的进度片段时发送 0.0–1.0（依赖 whisper-cli 输出格式）。
    pub async fn transcribe(
        &self,
        audio_data: &[u8],
        progress: Option<UnboundedSender<f32>>,
    ) -> anyhow::Result<TranscribeResult> {
        if audio_data.is_empty() {
            return Err(anyhow!("empty audio data"));
        }

        // Write audio to a temp file.
        let tmp_dir = tempfile::tempdir().context("create temp dir")?;
        let wav_path = tmp_dir.path().join("audio.wav");
        std::fs::write(&wav_path, audio_data).context("write temp wav file")?;

        // Find whisper-cli binary.
        let whisper_bin = find_whisper_binary(&self.whisper_path)?;

        // Expand tilde in model path.
        let model_path = expand_tilde(&self.model_path);

        let mut cmd = tokio::process::Command::new(&whisper_bin);
        cmd.arg("-m")
            .arg(model_path)
            .arg("-l")
            .arg(&self.language)
            .arg("-f")
            .arg(&wav_path)
            .arg("--no-timestamps")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().context("failed to spawn whisper-cli")?;
        let stdout = child.stdout.take().context("whisper-cli stdout")?;
        let stderr = child.stderr.take().context("whisper-cli stderr")?;

        let stderr_task = progress.map(|tx| {
            let re_percent = Regex::new(r"(\d{1,3})\s*%").expect("valid regex");
            let re_ratio = Regex::new(r"(\d+)\s*/\s*(\d+)").expect("valid regex");
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            if let Some(fr) = stderr_fraction(&line, &re_percent, &re_ratio) {
                                let _ = tx.send(fr);
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
        });

        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut s = stdout;
            s.read_to_end(&mut buf).await?;
            Ok::<_, std::io::Error>(buf)
        });

        let status = child.wait().await.context("failed to run whisper-cli")?;
        let stdout_buf = stdout_task
            .await
            .context("whisper stdout task")?
            .context("read whisper stdout")?;

        if let Some(t) = stderr_task {
            let _ = t.await;
        }

        if !status.success() {
            return Err(anyhow!(
                "whisper-cli failed (status {:?}); stdout: {}",
                status,
                String::from_utf8_lossy(&stdout_buf)
            ));
        }

        let text = String::from_utf8_lossy(&stdout_buf).trim().to_string();

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
/// 2. 捆绑安装的二进制文件
/// 3. 系统 PATH 中的 `whisper-cli` 和 `whisper-cpp`
fn find_whisper_binary(whisper_path: &str) -> anyhow::Result<std::path::PathBuf> {
    // No caching — config changes (whisper_path) should take effect on pipeline restart
    // without requiring an app restart.

    // 1. Use explicitly configured path.
    if !whisper_path.is_empty() {
        let path = std::path::Path::new(whisper_path);
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        return Err(anyhow!(
            "whisper-cli not found at configured path: {}",
            whisper_path
        ));
    }

    // 2. Check bundled location.
    if let Some(bundled) = crate::resource::bundled_bin("whisper-cli") {
        return Ok(bundled);
    }

    // 3. Search on PATH.
    let candidates = ["whisper-cli", "whisper-cpp"];
    for candidate in &candidates {
        if let Ok(found) = which_binary(candidate) {
            return Ok(found);
        }
    }

    Err(anyhow!(
        "whisper-cli not found — set whisper_path in config or add whisper-cli to PATH"
    ))
}

/// Search for a binary on the system PATH.
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

/// Unified transcription backend — dispatches between API and local engines.
#[derive(Clone)]
pub enum Transcriber {
    Api(WhisperApi),
    Local(LocalWhisper),
}

impl Transcriber {
    /// Transcribe audio data using the selected backend.
    pub async fn transcribe(
        &self,
        wav_data: &[u8],
        progress: Option<UnboundedSender<f32>>,
    ) -> anyhow::Result<TranscribeResult> {
        match self {
            Transcriber::Api(api) => {
                let _ = progress;
                api.transcribe(wav_data).await
            }
            Transcriber::Local(lw) => lw.transcribe(wav_data, progress).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stderr_fraction_parses_percent_and_ratio() {
        let re_p = Regex::new(r"(\d{1,3})\s*%").unwrap();
        let re_r = Regex::new(r"(\d+)\s*/\s*(\d+)").unwrap();
        assert_eq!(
            stderr_fraction("decode: 45% done", &re_p, &re_r),
            Some(0.45)
        );
        assert_eq!(stderr_fraction(" 12 / 40 ", &re_p, &re_r), Some(0.3));
    }

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
        let result = lw.transcribe(&[], None).await;
        assert!(result.is_err());
    }
}
