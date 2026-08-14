//! 语音识别模块。
//!
//! `Transcriber` trait 抽象所有后端：
//!
//! - `WhisperApi`：通过 HTTP multipart 请求调用兼容 OpenAI 的 Whisper API
//! - `MimoAsr`：通过 HTTP JSON 请求调用小米 MiMo-V2.5-ASR API
//! - `SherpaTranscriber`（`crate::sherpa`）：内嵌 sherpa-onnx 的本地 SenseVoice
//!
//! 所有实现都返回 `TranscribeResult`（文本 + 语言信息），进度通过闭包回调上报。

use crate::error::TranscriberError;
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

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

/// MiMo-V2.5-ASR API 语音识别器。
///
/// 通过 HTTP JSON 请求调用小米 MiMo ASR API，使用 OpenAI 兼容的 chat.completions 格式。
#[derive(Clone, Debug)]
pub struct MimoAsr {
    api_key: String,
    api_base_url: String,
    language: String,
    client: Client,
}

impl MimoAsr {
    pub fn new(
        api_key: String,
        api_base_url: String,
        language: String,
        timeout: Duration,
    ) -> Result<Self, TranscriberError> {
        let client = Client::builder().timeout(timeout).build().map_err(|e| {
            TranscriberError::HttpError(format!("failed to build HTTP client: {}", e))
        })?;
        Ok(Self {
            api_key,
            api_base_url,
            language,
            client,
        })
    }

    /// 通过 MiMo ASR API 识别音频数据，返回识别结果。
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

        let url = format!("{}/v1/chat/completions", self.api_base_url);

        // 将音频数据编码为 base64
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(audio_data);
        let audio_data_uri = format!("data:audio/wav;base64,{}", audio_base64);

        let language = if self.language.is_empty() {
            "auto".to_string()
        } else {
            self.language.clone()
        };

        let body = serde_json::json!({
            "model": "mimo-v2.5-asr",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_audio",
                            "input_audio": {
                                "data": audio_data_uri
                            }
                        }
                    ]
                }
            ],
            "asr_options": {
                "language": language
            }
        });

        let resp = self
            .client
            .post(&url)
            .header("api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
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

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TranscriberError::JsonError(e.to_string()))?;

        // 从 OpenAI 兼容的响应格式中提取文本
        let text = result
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err(TranscriberError::ApiError {
                status: 200,
                body: "empty transcription result".to_string(),
            });
        }

        // 尝试从响应中提取检测到的语言，否则使用请求参数（auto 时返回空）
        let detected_language = result
            .get("language")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_string();

        let language = if detected_language.is_empty() {
            if language == "auto" {
                String::new()
            } else {
                language
            }
        } else {
            detected_language
        };

        Ok(TranscribeResult { text, language })
    }
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

/// 统一的转写后端 trait——`on_progress` 回调由调用方提供，trait 表面不携带
/// 通道类型，新后端接入无需改动 trait。
pub trait Transcriber: Send + Sync {
    /// 转写 WAV 音频数据。
    ///
    /// `on_progress` 收到 0.0–1.0 的进度；无流式进度的后端成功时也应调用一次
    /// `1.0`，让 UI 收到最终一帧。
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0;
}

impl Transcriber for MimoAsr {
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        _on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0,
    {
        Box::pin(async move {
            let result = MimoAsr::transcribe(self, audio).await;
            if result.is_ok() {
                (_on_progress)(1.0);
            }
            result
        })
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

    fn mock_mimo_response(text: &str) -> String {
        serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": text
                }
            }],
            "language": "zh"
        })
        .to_string()
    }

    #[tokio::test]
    async fn test_mimo_transcribe_success() {
        let audio = [0u8; 44];
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(audio);
        let data_uri = format!("data:audio/wav;base64,{}", expected_b64);

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("api-key", "mimo-key")
            .match_body(mockito::Matcher::PartialJsonString(
                serde_json::json!({
                    "model": "mimo-v2.5-asr",
                    "messages": [{
                        "role": "user",
                        "content": [{
                            "type": "input_audio",
                            "input_audio": {"data": data_uri}
                        }]
                    }],
                    "asr_options": {"language": "zh"}
                })
                .to_string(),
            ))
            .with_status(200)
            .with_body(mock_mimo_response("你好世界"))
            .create_async()
            .await;

        let api = MimoAsr::new(
            "mimo-key".to_string(),
            server.url(),
            "zh".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = api.transcribe(&audio).await.unwrap();
        assert_eq!(result.text, "你好世界");
        assert_eq!(result.language, "zh");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_mimo_transcribe_empty_content_errors() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_body(mock_mimo_response(""))
            .create_async()
            .await;

        let api = MimoAsr::new(
            "mimo-key".to_string(),
            server.url(),
            "zh".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = api.transcribe(&[0u8; 44]).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TranscriberError::ApiError { status: 200, .. }
        ));
    }

    #[tokio::test]
    async fn test_mimo_transcribe_api_error_401() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(401)
            .with_body("unauthorized")
            .create_async()
            .await;

        let api = MimoAsr::new(
            "bad-key".to_string(),
            server.url(),
            "zh".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = api.transcribe(&[0u8; 44]).await;
        assert!(result.is_err());
    }
}
