//! Polisher API 协议类型定义。

use crate::error::PolisherError;
use serde::{Deserialize, Serialize};

/// API 协议类型。
#[derive(Debug, Clone, Copy)]
pub enum ApiProtocol {
    /// OpenAI 兼容接口（/v1/chat/completions）
    OpenAi,
    /// Anthropic Messages 接口（/v1/messages）
    Anthropic,
}

impl std::str::FromStr for ApiProtocol {
    type Err = PolisherError;

    fn from_str(s: &str) -> Result<Self, PolisherError> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(ApiProtocol::OpenAi),
            "anthropic" => Ok(ApiProtocol::Anthropic),
            _other => Err(PolisherError::UnknownProtocol {
                protocol: s.to_string(),
            }),
        }
    }
}

// --- OpenAI-compatible protocol ---

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

// --- Anthropic protocol ---

/// Anthropic Messages API 请求体。
#[derive(Debug, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<AnthropicMessage>,
    pub temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

/// Anthropic Messages API 响应。
#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicContent {
    pub text: String,
}
