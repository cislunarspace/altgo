//! 文本润色模块。
//!
//! 使用 LLM 对语音识别结果进行后期处理，支持 4 个润色级别：
//!
//! - `none`：不润色，直接返回原文
//! - `light`：修复标点和明显错别字
//! - `medium`：修复标点、错别字和语病，使语句更通顺
//! - `heavy`：重写为结构清晰、表达准确的文字
//!
//! 当语言为 `zh` 时，内置系统提示会约束输出为规范简体中文，并合并：材料概括类写作要求，以及本地安装的
//! **ljg-writes** / **ljg-plain**（lijigang/ljg-skills）中与「口语文本润色」相关的取向（非全文摘抄 skill 文件）。
//!
//! 使用兼容 OpenAI 的聊天 API，支持指数退避重试（最多 3 次）。

pub mod protocol;

use crate::error::PolisherError;
use reqwest::Client;
use std::time::Duration;

/// 重试延迟基数（毫秒），用于指数退避计算。
const RETRY_BASE_DELAY_MS: u64 = 500;

/// Generic retry helper with exponential backoff.
///
/// Retries the given async operation up to `max_retries` times.
/// Non-retryable errors (401, 403) are returned immediately.
async fn retry_with_backoff<F, Fut, T>(
    max_retries: u32,
    mut operation: F,
) -> Result<T, PolisherError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, PolisherError>>,
{
    let mut last_err = None;
    for attempt in 0..max_retries {
        if attempt > 0 {
            let delay = Duration::from_millis(RETRY_BASE_DELAY_MS * 2u64.pow(attempt - 1));
            tokio::time::sleep(delay).await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Check for non-retryable auth errors
                if matches!(
                    e,
                    PolisherError::ApiError { status: 401, .. }
                        | PolisherError::ApiError { status: 403, .. }
                ) {
                    return Err(e);
                }
                tracing::warn!(attempt, error = %e, "request failed");
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or(PolisherError::RetriesExhausted))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    /// 解析润色级别字符串，无效值回退到 `Medium`。
    pub fn effective(level_str: &str) -> Self {
        <Self as std::str::FromStr>::from_str(level_str).unwrap_or_else(|_| {
            tracing::warn!("invalid polish level '{level_str}', using medium");
            PolishLevel::Medium
        })
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

/// 中文润色时附加的写作与材料概括要求（与简体约束一并传入模型）。
const ZH_WRITE_GUIDANCE: &str = r#"注意写作的要求：要善于总结材料，这种总结就是将丰富的感性材料科学地加以概括，进行去粗取精、去伪存真、由此及彼、由表及里地加工改造。具体地讲，就是把材料搞全了、弄准了，把问题掰开了、揉碎了，把内在联系理清了、摆正了，这样才可以得到反映事物本质的真知和理论，才可以发现事物运动的规律。“关于写文章，请注意不要用过于夸大的修饰词，反而减损了力量。必须注意各种词语的逻辑界限和整篇文章的条理（也是逻辑问题）。废话应当尽量除去。”“文章写得通俗、亲切，由小讲到大，由近讲到远，引人入胜，这就很好。”“要采取和读者完全平等的态度。我们应该老老实实地办事，对事物有分析，写文章有说服力，不要靠装腔作势来吓人。”“总是先讲死人、外国人，这不好，应当从当前形势讲起。今后写文章要通俗，使工农都能接受。”"#;

/// 与语音转写润色相关的 **ljg-writes** / **ljg-plain** 取向（摘自用户本机 skill 要义，不含 Org/文件输出等仅技能执行用条款）。
const ZH_LJG_GUIDANCE: &str = r#"

【ljg-writes / ljg-plain（语音后润色适用；内化即可，勿输出本段标题或标签）】
姿态与诚实：心里是对一个具体的人讲，不是对抽象的「读者们」；不确定就保留不确定感，「大概七成」比空泛的「可能」诚实；忌群体代言、忌编经历、忌元评论（如「接下来我们讨论」）；禁止用「再深入一层」「最深的一层是」等宣告深度——深度靠下一句内容让人感受到，不靠自报。
语言：简洁、直白、质朴；能短则短；动词用准；砍掉机械连词（此外、另外）、形容词堆叠与软化套话（某种程度上、值得注意的是）；翻译腔句式（像英译中硬套）改成自然汉语；避免同一句式套话重复出现。
白话（ljg-plain 红线精神，按短文本尽量满足）：口语检验——像跟聪明朋友当面说吗；短词优先；一句一事，长句拆开；名词能具体则具体，动词有力，能删的形容词就删；开头少空泛铺陈与「自古以来」式引子；删开场白、拐杖词、宣传腔与夸大象征（标志着、见证了、充满活力等）；信任读者，不凑字数式手把手；专业词非必要不出现，必须出现时先大白话落地再点术语。
磨与中文：弱化学术/ AI 腔与「谁写都一样」的模板句；从句拆开、嵌套展平，发挥汉语意合；同一意思选最顺口的地道说法。"#;

fn get_system_prompt(level: PolishLevel, language: &str) -> String {
    let lang_name = match language {
        "zh" => "Simplified Chinese (简体中文, Mainland standard)",
        "en" => "English",
        "ja" => "日本語",
        "ko" => "한국어",
        "fr" => "français",
        "de" => "Deutsch",
        "es" => "español",
        _ => language,
    };

    // 明确约束简体，避免模型按「繁体/港台书面」习惯输出。
    let zh_script_rule = if language == "zh" {
        " For Chinese: output in Simplified Chinese only (大陆通用规范简体). Never use Traditional Chinese. If the input is Traditional, convert to Simplified. "
    } else {
        ""
    };

    // 写作与表达要求：全文融入用户提供的规范；轻量润色时强调不改结构、仅作最小必要调整。
    let zh_combined: String = if language == "zh" && !matches!(level, PolishLevel::None) {
        let intro = match level {
            PolishLevel::None => "",
            PolishLevel::Light => {
                " For light polish: do not restructure; tiny edits only. When text is Chinese, also heed these norms in spirit: "
            }
            PolishLevel::Medium | PolishLevel::Heavy => {
                " When output is Chinese, follow these writing norms: "
            }
        };
        format!("{intro}{ZH_WRITE_GUIDANCE}{ZH_LJG_GUIDANCE}")
    } else {
        String::new()
    };

    match level {
        PolishLevel::None => String::new(),
        PolishLevel::Light => format!(
            "You are a post-processing assistant for speech-to-text in {lang_name}. The user gives you raw speech recognition text in {lang_name}. Fix punctuation and obvious typos without changing the original meaning or word choices. Output only the corrected text with no explanation.{zh_script_rule}{zh_combined}"
        ),
        PolishLevel::Medium => format!(
            "You are a post-processing assistant for speech-to-text in {lang_name}. The user gives you raw speech recognition text in {lang_name}. Fix punctuation, typos, and grammar issues to make the text more fluent and natural, without changing the original meaning. Output only the corrected text with no explanation.{zh_script_rule}{zh_combined}"
        ),
        PolishLevel::Heavy => format!(
            "You are a post-processing assistant for speech-to-text in {lang_name}. The user gives you raw speech recognition text in {lang_name}. Rewrite it into well-structured, clearly expressed text. You may adjust word order and phrasing, but preserve the core meaning. Output only the rewritten text with no explanation.{zh_script_rule}{zh_combined}"
        ),
    }
}

// ---------------------------------------------------------------------------
// SystemPromptSource trait and implementations
// ---------------------------------------------------------------------------

/// 抽象 system prompt 来源，消除 `polish()` 内部的 fallback 链。
///
/// 由 `LLMFormatter` 持有，使 prompt 选择逻辑可在测试中替换。
pub trait SystemPromptSource: Send + Sync {
    /// 获取指定级别和语言的 system prompt。
    fn get_prompt(
        &self,
        level: PolishLevel,
        language: &str,
    ) -> Result<String, crate::prompt_store::PromptError>;

    /// 支持 clone 为 trait object（用于 `LLMFormatter::Clone`）。
    fn clone_box(&self) -> Box<dyn SystemPromptSource>;
}

/// 基于 `PromptStore` 的 prompt 来源。
pub struct PromptStoreSource {
    store: crate::prompt_store::PromptStore,
}

impl PromptStoreSource {
    pub fn new(store: crate::prompt_store::PromptStore) -> Self {
        Self { store }
    }
}

impl SystemPromptSource for PromptStoreSource {
    fn get_prompt(
        &self,
        level: PolishLevel,
        _language: &str,
    ) -> Result<String, crate::prompt_store::PromptError> {
        self.store.get_system_prompt(level)
    }

    fn clone_box(&self) -> Box<dyn SystemPromptSource> {
        Box::new(PromptStoreSource {
            store: self.store.clone(),
        })
    }
}

/// 用户自定义 prompt 来源（来自 `config.polisher.system_prompt`）。
pub struct CustomSource {
    prompt: String,
}

impl CustomSource {
    pub fn new(prompt: String) -> Self {
        Self { prompt }
    }
}

impl SystemPromptSource for CustomSource {
    fn get_prompt(
        &self,
        _level: PolishLevel,
        _language: &str,
    ) -> Result<String, crate::prompt_store::PromptError> {
        Ok(self.prompt.clone())
    }

    fn clone_box(&self) -> Box<dyn SystemPromptSource> {
        Box::new(CustomSource {
            prompt: self.prompt.clone(),
        })
    }
}

/// LLM 文本润色器。
///
/// 支持 OpenAI 和 Anthropic 两种 API 协议，
/// 支持指数退避重试（最多 3 次）。
///
/// System prompt 通过 `SystemPromptSource` trait 注入；`prompt_source` 为 `None` 时
/// `polish()` 内部用内置 hardcoded prompt 兜底。
pub struct LLMFormatter {
    api_key: String,
    api_base_url: String,
    model: String,
    client: Client,
    max_retries: u32,
    max_tokens: u32,
    protocol: protocol::ApiProtocol,
    temperature: f32,
    language: String,
    prompt_source: Option<Box<dyn SystemPromptSource>>,
}

impl Clone for LLMFormatter {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            api_base_url: self.api_base_url.clone(),
            model: self.model.clone(),
            client: self.client.clone(),
            max_retries: self.max_retries,
            max_tokens: self.max_tokens,
            protocol: self.protocol,
            temperature: self.temperature,
            language: self.language.clone(),
            prompt_source: self.prompt_source.as_ref().map(|s| s.clone_box()),
        }
    }
}

impl std::fmt::Debug for LLMFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LLMFormatter")
            .field("model", &self.model)
            .field("protocol", &self.protocol)
            .field("language", &self.language)
            .finish()
    }
}

impl TryFrom<&crate::config::Config> for LLMFormatter {
    type Error = PolisherError;

    fn try_from(cfg: &crate::config::Config) -> Result<Self, Self::Error> {
        Self::from_config(&cfg.polisher, &cfg.transcriber.language)
    }
}

impl LLMFormatter {
    /// Create LLMFormatter from config sections.
    pub fn from_config(
        polisher: &crate::config::PolisherConfig,
        language: &str,
    ) -> Result<Self, PolisherError> {
        let protocol = polisher
            .protocol
            .parse::<protocol::ApiProtocol>()
            .map_err(|_| PolisherError::UnknownProtocol {
                protocol: polisher.protocol.clone(),
            })?;
        Self::with_config(
            polisher.api_key.clone(),
            polisher.api_base_url.clone(),
            polisher.model.clone(),
            polisher.timeout,
            polisher.max_tokens,
            protocol,
            polisher.temperature,
            language.to_string(),
        )
    }

    /// 共享工厂：从 Config 一次性构造带全部 prompt source 的 LLMFormatter。
    ///
    /// 实时管道（`voice_pipeline::builder::build_polisher`）与 IPC handler
    /// （`cmd::polish_history_entry`）都通过此构造，确保两条路径走相同的
    /// prompt 解析：PromptStore → Custom → hardcoded fallback。
    pub fn from_config_with_sources(
        cfg: &crate::config::Config,
    ) -> Result<Self, PolisherError> {
        let mut formatter = Self::from_config(&cfg.polisher, &cfg.transcriber.language)?;
        formatter.prompt_source = build_prompt_source_chain(cfg);
        Ok(formatter)
    }

    #[cfg(test)]
    pub fn new(
        api_key: String,
        api_base_url: String,
        model: String,
        timeout: Duration,
    ) -> Result<Self, PolisherError> {
        Self::with_config(
            api_key,
            api_base_url,
            model,
            timeout,
            1024,
            protocol::ApiProtocol::OpenAi,
            0.3,
            "zh".to_string(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_config(
        api_key: String,
        api_base_url: String,
        model: String,
        timeout: Duration,
        max_tokens: u32,
        protocol: protocol::ApiProtocol,
        temperature: f32,
        language: String,
    ) -> Result<Self, PolisherError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| PolisherError::HttpError(format!("failed to build HTTP client: {}", e)))?;
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
            prompt_source: None,
        })
    }

    /// Sets the prompt source for system prompt resolution. `None` 表示使用内置 hardcoded prompt。
    pub fn with_prompt_source(
        mut self,
        source: Option<Box<dyn SystemPromptSource>>,
    ) -> Self {
        self.prompt_source = source;
        self
    }

    /// 使用 LLM 润色文本。
    ///
    /// 如果级别为 `None` 或文本为空，直接返回原文。
    /// 润色失败时返回错误。
    pub async fn polish(&self, text: &str, level: PolishLevel) -> Result<String, PolisherError> {
        if matches!(level, PolishLevel::None) || text.is_empty() {
            return Ok(text.to_string());
        }

        let system_prompt = self
            .prompt_source
            .as_ref()
            .and_then(|s| s.get_prompt(level, &self.language).ok())
            .unwrap_or_else(|| get_system_prompt(level, &self.language));

        retry_with_backoff(self.max_retries, || async {
            match self.protocol {
                protocol::ApiProtocol::OpenAi => {
                    let body = protocol::ChatRequest {
                        model: self.model.clone(),
                        messages: vec![
                            protocol::ChatMessage {
                                role: "system".to_string(),
                                content: system_prompt.clone(),
                            },
                            protocol::ChatMessage {
                                role: "user".to_string(),
                                content: text.to_string(),
                            },
                        ],
                        temperature: self.temperature,
                        max_tokens: self.max_tokens,
                    };
                    self.do_openai_request(&body).await
                }
                protocol::ApiProtocol::Anthropic => {
                    let body = protocol::AnthropicRequest {
                        model: self.model.clone(),
                        max_tokens: self.max_tokens,
                        system: system_prompt.clone(),
                        messages: vec![protocol::AnthropicMessage {
                            role: "user".to_string(),
                            content: text.to_string(),
                        }],
                        temperature: self.temperature,
                    };
                    self.do_anthropic_request(&body).await
                }
            }
        })
        .await
    }

    async fn do_openai_request(
        &self,
        body: &protocol::ChatRequest,
    ) -> Result<String, PolisherError> {
        let url = format!("{}/v1/chat/completions", self.api_base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(body)
            .send()
            .await
            .map_err(|e| PolisherError::HttpError(e.to_string()))?;

        let status = resp.status().as_u16();
        if status == 429 {
            return Err(PolisherError::RateLimited);
        }
        if !resp.status().is_success() {
            let resp_body = resp
                .text()
                .await
                .unwrap_or_else(|_| "failed to read error body".to_string());
            return Err(PolisherError::ApiError {
                status,
                body: resp_body,
            });
        }

        let chat_resp: protocol::ChatResponse = resp
            .json()
            .await
            .map_err(|e| PolisherError::JsonError(e.to_string()))?;
        chat_resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or(PolisherError::EmptyResponse)
    }

    async fn do_anthropic_request(
        &self,
        body: &protocol::AnthropicRequest,
    ) -> Result<String, PolisherError> {
        let url = format!("{}/v1/messages", self.api_base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(body)
            .send()
            .await
            .map_err(|e| PolisherError::HttpError(e.to_string()))?;

        let status = resp.status().as_u16();
        if status == 429 {
            return Err(PolisherError::RateLimited);
        }
        if !resp.status().is_success() {
            let resp_body = resp
                .text()
                .await
                .unwrap_or_else(|_| "failed to read error body".to_string());
            return Err(PolisherError::ApiError {
                status,
                body: resp_body,
            });
        }

        let anthropic_resp: protocol::AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| PolisherError::JsonError(e.to_string()))?;
        anthropic_resp
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .ok_or(PolisherError::EmptyResponse)
    }
}

/// 抽出 builder.rs 里的 prompt source chain 构造逻辑。所有调用方共享一份。
///
/// 优先级：PromptStore（`resources/prompts` 加载成功）→ Custom（`system_prompt` 非空）→ `None`。
/// `None` 时调用方 `polish()` 用内置 hardcoded prompt 兜底。
fn build_prompt_source_chain(
    cfg: &crate::config::Config,
) -> Option<Box<dyn SystemPromptSource>> {
    let store_source: Option<Box<dyn SystemPromptSource>> =
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("resources/prompts")))
            .or_else(|| Some(std::path::PathBuf::from("resources/prompts")))
            .filter(|dir| dir.exists())
            .and_then(|dir| {
                let store = crate::prompt_store::PromptStore::new(dir);
                match store.ensure_loaded() {
                    Ok(()) => {
                        tracing::info!("PromptStore loaded successfully");
                        Some(Box::new(PromptStoreSource::new(store))
                            as Box<dyn SystemPromptSource>)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to load prompts from PromptStore");
                        None
                    }
                }
            });

    let custom_source: Option<Box<dyn SystemPromptSource>> =
        if !cfg.polisher.system_prompt.is_empty() {
            Some(Box::new(CustomSource::new(cfg.polisher.system_prompt.clone()))
                as Box<dyn SystemPromptSource>)
        } else {
            None
        };

    store_source.or(custom_source)
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
            protocol::ApiProtocol::OpenAi,
            0.3,
            "zh".to_string(),
        )
        .unwrap();
        let _ = formatter.polish("test", PolishLevel::Light).await;
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_polish_api_error_401_no_retry() {
        // 401 should NOT be retried — it's an auth error, not a transient failure.
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(401)
            .with_body("unauthorized")
            .expect(1)
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
