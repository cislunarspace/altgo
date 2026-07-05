//! 语音识别模块。
//!
//! `Transcriber` trait 抽象所有后端：
//!
//! - `WhisperApi`：通过 HTTP multipart 请求调用兼容 OpenAI 的 Whisper API
//! - `LocalWhisper`：通过子进程调用本地 `whisper-cli` 二进制文件
//! - `ResidentWhisper`：常驻 whisper-server + whisper-cli 回退
//!
//! 所有实现都返回 `TranscribeResult`（文本 + 语言信息），进度通过闭包回调上报。

use crate::error::TranscriberError;
use crate::resource::expand_tilde;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
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

/// 语音识别结果。
#[derive(Debug)]
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
#[derive(Clone, Debug)]
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
    ) -> Result<Self, TranscriberError> {
        let client = Client::builder().timeout(timeout).build().map_err(|e| {
            TranscriberError::HttpError(format!("failed to build HTTP client: {}", e))
        })?;
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
    pub async fn transcribe(
        &self,
        audio_data: &[u8],
    ) -> Result<TranscribeResult, TranscriberError> {
        if audio_data.is_empty() {
            return Err(TranscriberError::EmptyAudio);
        }
        if self.api_key.is_empty() {
            return Err(TranscriberError::MissingApiKey);
        }

        let url = format!("{}/v1/audio/transcriptions", self.api_base_url);

        let audio_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| TranscriberError::HttpError(e.to_string()))?;

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
            .map_err(|e| TranscriberError::HttpError(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "failed to read error body".to_string());
            return Err(TranscriberError::ApiError {
                status: status.as_u16(),
                body,
            });
        }

        let result: WhisperResponse = resp
            .json()
            .await
            .map_err(|e| TranscriberError::JsonError(e.to_string()))?;

        Ok(TranscribeResult {
            text: result.text,
            language: result.language.unwrap_or_default(),
        })
    }
}

/// 本地 whisper.cpp 语音识别器。
///
/// 通过子进程调用 `whisper-cli` 二进制文件，避免 FFI 构建复杂性。
#[derive(Clone, Debug)]
pub struct LocalWhisper {
    model_path: String,
    language: String,
    whisper_path: String,
    /// 线程数；`0` 表示按 CPU 并行度自动取满（whisper 默认仅 min(4, hw)）。
    threads: u32,
    /// beam search 宽度；`<= 1` 时走贪心（最快）。
    beam_size: u32,
}

impl LocalWhisper {
    /// 创建新的本地语音识别器。
    ///
    /// `model_path`：whisper 模型文件路径
    /// `language`：语言代码
    /// `whisper_path`：whisper-cli 二进制文件路径（为空时自动查找）
    /// `threads`：线程数（`0` = 自动取满 CPU 核数）
    /// `beam_size`：beam search 宽度（`<= 1` = 贪心，最快）
    pub fn new(
        model_path: String,
        language: String,
        whisper_path: String,
        threads: u32,
        beam_size: u32,
    ) -> Self {
        Self {
            model_path,
            language,
            whisper_path,
            threads,
            beam_size,
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
    ) -> Result<TranscribeResult, TranscriberError> {
        if audio_data.is_empty() {
            return Err(TranscriberError::EmptyAudio);
        }

        // Write audio to a temp file.
        let tmp_dir = std::env::temp_dir().join(format!("altgo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&tmp_dir).map_err(|e| TranscriberError::WhisperCliFailed {
            code: -1,
            output: format!("create temp dir: {}", e),
        })?;
        let wav_path = tmp_dir.join("audio.wav");
        // ponytail: cleanup tmp_dir on write failure (TempDir::Drop did this automatically)
        let write_result = std::fs::write(&wav_path, audio_data).map_err(|e| {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            TranscriberError::WhisperCliFailed {
                code: -1,
                output: format!("write temp wav file: {}", e),
            }
        });
        write_result?;

        let result = self.do_transcribe(&wav_path, progress).await;
        // ponytail: best-effort cleanup, matches old tempfile::TempDir behavior
        let _ = std::fs::remove_dir_all(&tmp_dir);
        result
    }

    async fn do_transcribe(
        &self,
        wav_path: &std::path::Path,
        progress: Option<UnboundedSender<f32>>,
    ) -> Result<TranscribeResult, TranscriberError> {
        // Find whisper-cli binary.
        let whisper_bin = find_whisper_binary(&self.whisper_path)?;

        // Expand tilde in model path.
        let model_path = expand_tilde(&self.model_path);

        let mut cmd = tokio::process::Command::new(&whisper_bin);
        cmd.arg("-m")
            .arg(model_path)
            .arg("-l")
            .arg(&self.language)
            .arg("-t")
            .arg(crate::whisper_server::effective_threads(self.threads).to_string())
            .arg("-f")
            .arg(wav_path)
            .arg("--no-timestamps")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // beam_size <= 1 时使用 whisper 默认贪心；> 1 才显式开 beam search。
        if self.beam_size > 1 {
            cmd.arg("-bs").arg(self.beam_size.to_string());
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| TranscriberError::WhisperCliFailed {
                code: -1,
                output: format!("failed to spawn whisper-cli: {}", e),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TranscriberError::WhisperCliFailed {
                code: -1,
                output: "whisper-cli stdout unavailable".to_string(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TranscriberError::WhisperCliFailed {
                code: -1,
                output: "whisper-cli stderr unavailable".to_string(),
            })?;

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

        let status = child
            .wait()
            .await
            .map_err(|e| TranscriberError::WhisperCliFailed {
                code: -1,
                output: format!("failed to run whisper-cli: {}", e),
            })?;
        let stdout_buf = stdout_task
            .await
            .map_err(|e| TranscriberError::WhisperCliFailed {
                code: -1,
                output: format!("whisper stdout task: {}", e),
            })?
            .map_err(|e| TranscriberError::WhisperCliFailed {
                code: -1,
                output: format!("read whisper stdout: {}", e),
            })?;

        if let Some(t) = stderr_task {
            let _ = t.await;
        }

        if !status.success() {
            return Err(TranscriberError::WhisperCliFailed {
                code: status.code().unwrap_or(-1),
                output: String::from_utf8_lossy(&stdout_buf).to_string(),
            });
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
fn find_whisper_binary(whisper_path: &str) -> Result<std::path::PathBuf, TranscriberError> {
    // No caching — config changes (whisper_path) should take effect on pipeline restart
    // without requiring an app restart.

    // 1. Use explicitly configured path.
    if !whisper_path.is_empty() {
        let path = std::path::Path::new(whisper_path);
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        return Err(TranscriberError::WhisperCliNotFound {
            path: whisper_path.to_string(),
        });
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

    Err(TranscriberError::WhisperCliNotFound {
        path: "whisper-cli not found — set whisper_path in config or add whisper-cli to PATH"
            .to_string(),
    })
}

/// Search for a binary on the system PATH.
pub(crate) fn which_binary(name: &str) -> Result<std::path::PathBuf, TranscriberError> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .map_err(|e| TranscriberError::WhisperCliNotFound {
            path: format!("which command failed: {}", e),
        })?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(TranscriberError::WhisperCliNotFound {
        path: format!("{} not found on PATH", name),
    })
}

/// Unified transcription backend — dispatches between API, resident server, and one-shot local engines.
///
/// Progress is reported through the `on_progress` callback (a function pointer
/// pointing to a closure the caller controls); the trait surface stays free of
/// channel types so new backends can be plugged in without touching the trait.
pub trait Transcriber: Send + Sync {
    /// Transcribe WAV audio data.
    ///
    /// `on_progress` receives 0.0–1.0 fractions as the backend makes progress;
    /// backends without streaming progress should still call it once with `1.0`
    /// upon success so the UI gets a final tick.
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0;
}

impl Transcriber for WhisperApi {
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        _on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0,
    {
        Box::pin(async move { WhisperApi::transcribe(self, audio).await })
    }
}

impl Transcriber for LocalWhisper {
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0,
    {
        Box::pin(async move {
            // Bridge the callback back into the channel the inner implementation
            // still consumes. One unbounded sender; one forwarder task per call.
            let cb = Arc::clone(&on_progress);
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
            let forward = tokio::spawn(async move {
                while let Some(fr) = rx.recv().await {
                    (cb)(fr);
                }
            });
            let result = LocalWhisper::transcribe(self, audio, Some(tx)).await;
            let _ = forward.await;
            if result.is_ok() {
                (on_progress)(1.0);
            }
            result
        })
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
        let err = result.unwrap_err();
        assert!(matches!(err, TranscriberError::EmptyAudio));
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
            0,
            0,
        );
        let result = lw.transcribe(&[], None).await;
        assert!(result.is_err());
    }
}
