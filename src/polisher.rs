use anyhow::{anyhow, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Polish level controlling how aggressively the LLM rewrites text.
#[derive(Debug, Clone, Copy)]
pub enum PolishLevel {
    None,
    Light,
    Medium,
    Heavy,
}

impl PolishLevel {
    #[allow(dead_code)]
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
        match s.to_lowercase().as_str() {
            "none" => Ok(PolishLevel::None),
            "light" => Ok(PolishLevel::Light),
            "medium" => Ok(PolishLevel::Medium),
            "heavy" => Ok(PolishLevel::Heavy),
            other => Err(format!("unknown polish level: {other}")),
        }
    }
}

fn get_system_prompt(level: PolishLevel) -> &'static str {
    match level {
        PolishLevel::None => "",
        PolishLevel::Light => "你是一个中文语音转文字的后期处理助手。用户给你一段语音识别的原始文本，你需要修复其中的标点符号和明显的错别字，但不改变文本的原本意思和用词。只输出修改后的文本，不要任何解释。",
        PolishLevel::Medium => "你是一个中文语音转文字的后期处理助手。用户给你一段语音识别的原始文本，你需要修复标点符号、错别字和语病，让语句更通顺流畅，但不改变文本的原本意思。只输出修改后的文本，不要任何解释。",
        PolishLevel::Heavy => "你是一个中文语音转文字的后期处理助手。用户给你一段语音识别的原始文本，你需要将其重写为结构清晰、表达准确的文字。可以适当调整语序和措辞，但保留核心意思不变。只输出修改后的文本，不要任何解释。",
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

#[derive(Clone)]
pub struct LLMFormatter {
    api_key: String,
    api_base_url: String,
    model: String,
    client: Client,
    max_retries: u32,
}

impl LLMFormatter {
    pub fn new(api_key: String, api_base_url: String, model: String, timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            api_key,
            api_base_url,
            model,
            client,
            max_retries: 3,
        }
    }

    /// Polish text using the configured LLM. Returns the original text if
    /// level is None or if polishing fails.
    pub async fn polish(&self, text: &str, level: PolishLevel) -> anyhow::Result<String> {
        if matches!(level, PolishLevel::None) || text.is_empty() {
            return Ok(text.to_string());
        }

        let system_prompt = get_system_prompt(level);
        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: 0.3,
            max_tokens: 1024,
        };

        let mut last_err = None;
        for attempt in 0..self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            match self.do_request(&body).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "polish request failed");
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("all retry attempts exhausted")))
    }

    async fn do_request(&self, body: &ChatRequest) -> anyhow::Result<String> {
        let url = format!("{}/v1/chat/completions", self.api_base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(body)
            .send()
            .await
            .context("LLM API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("LLM API returned {}: {}", status, body));
        }

        let chat_resp: ChatResponse = resp.json().await.context("failed to parse LLM response")?;

        chat_resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| anyhow!("LLM returned empty choices"))
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
        );
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
        );
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
        );
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
        let expected_system = get_system_prompt(PolishLevel::Light);
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

        let formatter = LLMFormatter::new(
            "key".to_string(),
            server.url(),
            "model".to_string(),
            Duration::from_secs(5),
        );
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
        );
        let result = formatter.polish("test", PolishLevel::Medium).await;
        assert!(result.is_err());
    }
}
