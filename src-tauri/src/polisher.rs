//! 文本润色模块。
//!
//! 使用 LLM 对语音识别结果进行后期处理，支持 4 个润色级别：
//!
//! - `none`：不润色，直接返回原文
//! - `light`：修复标点和明显错别字
//! - `medium`：修复标点、错别字和语病，使语句更通顺
//! - `heavy`：重写为结构清晰、表达准确的文字
//!
//! 使用兼容 OpenAI 的聊天 API，支持指数退避重试（最多 3 次）。

use anyhow::{anyhow, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 重试延迟基数（毫秒），用于指数退避计算。
const RETRY_BASE_DELAY_MS: u64 = 500;

struct HttpStatusError(u16);

impl std::fmt::Display for HttpStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}", self.0)
    }
}

impl std::fmt::Debug for HttpStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HttpStatusError({})", self.0)
    }
}

impl std::error::Error for HttpStatusError {}

#[derive(Debug, Clone, Copy)]
pub enum PolishLevel {
    /// 不润色
    None,
    /// 轻度润色：修复标点和错别字
    Light,
    /// 中度润色：修复标点、错别字和语病
    Medium,
    /// 重度润色：重写为结构清晰的文字
    Heavy,
}

impl PolishLevel {
    #[cfg(test)]
    fn as_str(self) -> &'static str {
        match self {
            PolishLevel::None => "none",
            PolishLevel::Light => "light",
            PolishLevel::Medium => "medium",
            PolishLevel::Heavy => "heavy",
        }
    }
}

impl std::str::FromStr for PolishLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.eq_ignore_ascii_case("none") => Ok(PolishLevel::None),
            s if s.eq_ignore_ascii_case("light") => Ok(PolishLevel::Light),
            s if s.eq_ignore_ascii_case("medium") => Ok(PolishLevel::Medium),
            s if s.eq_ignore_ascii_case("heavy") => Ok(PolishLevel::Heavy),
            other => Err(format!("unknown polish level: {other}")),
        }
    }
}

fn get_system_prompt(level: PolishLevel, language: &str) -> String {
    let lang_name = match language {
        "zh" => "中文",
        "en" => "English",
        "ja" => "日本語",
        "ko" => "한국어",
        "fr" => "français",
        "de" => "Deutsch",
        "es" => "español",
        _ => language,
    };

    match level {
        PolishLevel::None => String::new(),
        PolishLevel::Light => format!("You are a post-processing assistant for speech-to-text in {lang_name}. The user gives you raw speech recognition text in {lang_name}. Fix punctuation and obvious typos without changing the original meaning or word choices. Output only the corrected text with no explanation."),
        PolishLevel::Medium => format!("You are a post-processing assistant for speech-to-text in {lang_name}. The user gives you raw speech recognition text in {lang_name}. Fix punctuation, typos, and grammar issues to make the text more fluent and natural, without changing the original meaning. Output only the corrected text with no explanation."),
        PolishLevel::Heavy => format!("You are a post-processing assistant for speech-to-text in {lang_name}. The user gives you raw speech recognition text in {lang_name}. Rewrite it into well-structured, clearly expressed text. You may adjust word order and phrasing, but preserve the core meaning. Output only the rewritten text with no explanation."),
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

/// Anthropic Messages API 请求体。
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Anthropic Messages API 响应。
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

/// API 协议类型。
#[derive(Debug, Clone, Copy)]
pub enum ApiProtocol {
    /// OpenAI 兼容接口（/v1/chat/completions）
    OpenAi,
    /// Anthropic Messages 接口（/v1/messages）
    Anthropic,
}

impl std::str::FromStr for ApiProtocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(ApiProtocol::OpenAi),
            "anthropic" => Ok(ApiProtocol::Anthropic),
            other => Err(anyhow!(
                "unknown polisher protocol: '{}'. Use 'openai' or 'anthropic'",
                other
            )),
        }
    }
}

impl ApiProtocol {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        <Self as std::str::FromStr>::from_str(s)
    }
}

/// LLM 文本润色器。
///
/// 支持 OpenAI 和 Anthropic 两种 API 协议，
/// 支持指数退避重试（最多 3 次）。
#[derive(Clone)]
pub struct LLMFormatter {
    api_key: String,
    api_base_url: String,
    model: String,
    client: Client,
    max_retries: u32,
    max_tokens: u32,
    protocol: ApiProtocol,
    temperature: f32,
    language: String,
    custom_system_prompt: String,
}

impl LLMFormatter {
    #[allow(dead_code)]
    pub fn new(
        api_key: String,
        api_base_url: String,
        model: String,
        timeout: Duration,
    ) -> anyhow::Result<Self> {
        Self::with_config(
            api_key,
            api_base_url,
            model,
            timeout,
            1024,
            ApiProtocol::OpenAi,
            0.3,
            "zh".to_string(),
            String::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_config(
        api_key: String,
        api_base_url: String,
        model: String,
        timeout: Duration,
        max_tokens: u32,
        protocol: ApiProtocol,
        temperature: f32,
        language: String,
        custom_system_prompt: String,
    ) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .context("failed to build HTTP client for LLMFormatter")?;
        Ok(Self {
            api_key,
            api_base_url,
            model,
            client,
            max_retries: 3,
            max_tokens,
            protocol,
            temperature,
            language,
            custom_system_prompt,
        })
    }

    /// 使用 LLM 润色文本。
    ///
    /// 如果级别为 `None` 或文本为空，直接返回原文。
    /// 润色失败时返回错误。
    pub async fn polish(&self, text: &str, level: PolishLevel) -> anyhow::Result<String> {
        if matches!(level, PolishLevel::None) || text.is_empty() {
            return Ok(text.to_string());
        }

        let system_prompt = if self.custom_system_prompt.is_empty() {
            get_system_prompt(level, &self.language)
        } else {
            self.custom_system_prompt.clone()
        };

        let mut last_err = None;
        for attempt in 0..self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(RETRY_BASE_DELAY_MS * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            let result = match self.protocol {
                ApiProtocol::OpenAi => {
                    let body = ChatRequest {
                        model: self.model.clone(),
                        messages: vec![
                            ChatMessage {
                                role: "system".to_string(),
                                content: system_prompt.clone(),
                            },
                            ChatMessage {
                                role: "user".to_string(),
                                content: text.to_string(),
                            },
                        ],
                        temperature: self.temperature,
                        max_tokens: self.max_tokens,
                    };
                    self.do_openai_request(&body).await
                }
                ApiProtocol::Anthropic => {
                    let body = AnthropicRequest {
                        model: self.model.clone(),
                        max_tokens: self.max_tokens,
                        system: system_prompt.clone(),
                        messages: vec![AnthropicMessage {
                            role: "user".to_string(),
                            content: text.to_string(),
                        }],
                        temperature: self.temperature,
                    };
                    self.do_anthropic_request(&body).await
                }
            };

            match result {
                Ok(r) => return Ok(r),
                Err(e) => {
                    if let Some(status) = e.downcast_ref::<HttpStatusError>() {
                        if status.0 == 401 || status.0 == 403 {
                            return Err(e);
                        }
                    }
                    let err_str = e.to_string();
                    if err_str.contains("401") || err_str.contains("403") {
                        return Err(e);
                    }
                    tracing::warn!(attempt, error = %e, "polish request failed");
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("all retry attempts exhausted")))
    }

    async fn do_openai_request(&self, body: &ChatRequest) -> anyhow::Result<String> {
        let url = format!("{}/v1/chat/completions", self.api_base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(body)
            .send()
            .await
            .context("OpenAI API request failed")?;

        let status = resp.status().as_u16();
        if status == 429 {
            return Err(anyhow!("rate limited"));
        }
        if !resp.status().is_success() {
            let resp_body = resp.text().await.context("failed to read API error body")?;
            let mut err = anyhow!("LLM API returned {}: {}", status, resp_body);
            if status == 401 || status == 403 {
                err = err.context(HttpStatusError(status));
            }
            return Err(err);
        }

        let chat_resp: ChatResponse = resp.json().await.context("failed to parse LLM response")?;
        chat_resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| anyhow!("LLM returned empty choices"))
    }

    async fn do_anthropic_request(&self, body: &AnthropicRequest) -> anyhow::Result<String> {
        let url = format!("{}/v1/messages", self.api_base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(body)
            .send()
            .await
            .context("Anthropic API request failed")?;

        let status = resp.status().as_u16();
        if status == 429 {
            return Err(anyhow!("rate limited"));
        }
        if !resp.status().is_success() {
            let resp_body = resp.text().await.context("failed to read API error body")?;
            let mut err = anyhow!("LLM API returned {}: {}", status, resp_body);
            if status == 401 || status == 403 {
                err = err.context(HttpStatusError(status));
            }
            return Err(err);
        }

        let anthropic_resp: AnthropicResponse = resp
            .json()
            .await
            .context("failed to parse Anthropic response")?;
        anthropic_resp
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .ok_or_else(|| anyhow!("Anthropic returned empty content"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_polish_level_from_str() {
        assert!(matches!(
            PolishLevel::from_str("none").unwrap(),
            PolishLevel::None
        ));
        assert!(matches!(
            PolishLevel::from_str("light").unwrap(),
            PolishLevel::Light
        ));
        assert!(matches!(
            PolishLevel::from_str("medium").unwrap(),
            PolishLevel::Medium
        ));
        assert!(matches!(
            PolishLevel::from_str("heavy").unwrap(),
            PolishLevel::Heavy
        ));
        assert!(PolishLevel::from_str("unknown").is_err());
    }

    #[test]
    fn test_polish_level_as_str() {
        assert_eq!(PolishLevel::None.as_str(), "none");
        assert_eq!(PolishLevel::Light.as_str(), "light");
        assert_eq!(PolishLevel::Medium.as_str(), "medium");
        assert_eq!(PolishLevel::Heavy.as_str(), "heavy");
    }

    #[tokio::test]
    async fn test_polish_none_skips_api() {
        let formatter = LLMFormatter::new(
            "key".to_string(),
            "http://localhost".to_string(),
            "model".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = formatter.polish("hello", PolishLevel::None).await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_polish_empty_skips_api() {
        let formatter = LLMFormatter::new(
            "key".to_string(),
            "http://localhost".to_string(),
            "model".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = formatter.polish("", PolishLevel::Medium).await.unwrap();
        assert_eq!(result, "");
    }

    fn mock_success_response(content: &str) -> String {
        serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": content
                }
            }]
        })
        .to_string()
    }

    #[tokio::test]
    async fn test_polish_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-key")
            .with_status(200)
            .with_body(&mock_success_response("润色后的文本"))
            .create_async()
            .await;

        let formatter = LLMFormatter::new(
            "test-key".to_string(),
            server.url(),
            "deepseek-chat".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = formatter
            .polish("原始文本", PolishLevel::Medium)
            .await
            .unwrap();
        assert_eq!(result, "润色后的文本");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_polish_sends_correct_prompt_for_light() {
        let mut server = mockito::Server::new_async().await;
        let expected_system = get_system_prompt(PolishLevel::Light, "zh");
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_body(mockito::Matcher::PartialJsonString(
                serde_json::json!({
                    "messages": [
                        {"role": "system", "content": expected_system},
                        {"role": "user", "content": "test"}
                    ]
                })
                .to_string(),
            ))
            .with_status(200)
            .with_body(&mock_success_response("ok"))
            .create_async()
            .await;

        let formatter = LLMFormatter::with_config(
            "key".to_string(),
            server.url(),
            "model".to_string(),
            Duration::from_secs(5),
            1024,
            ApiProtocol::OpenAi,
            0.3,
            "zh".to_string(),
            String::new(),
        )
        .unwrap();
        let _ = formatter.polish("test", PolishLevel::Light).await;
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_polish_api_error_retries_and_fails() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(401)
            .with_body("unauthorized")
            .expect(3)
            .create_async()
            .await;

        let formatter = LLMFormatter::new(
            "bad-key".to_string(),
            server.url(),
            "model".to_string(),
            Duration::from_secs(5),
        )
        .unwrap();
        let result = formatter.polish("test", PolishLevel::Medium).await;
        assert!(result.is_err());
    }
}
