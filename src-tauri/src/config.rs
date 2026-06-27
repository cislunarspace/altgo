//! 配置加载模块。
//!
//! 从 TOML 文件加载 altgo 配置，所有字段均通过 `serde(default)` 提供默认值，
//! 因此部分配置文件也可以正常工作。
//!
//! API 密钥支持通过环境变量覆盖：
//! - `ALTGO_TRANSCRIBER_API_KEY` — 覆盖语音识别 API 密钥
//! - `ALTGO_POLISHER_API_KEY` — 覆盖文本润色 API 密钥
//!
//! 默认配置路径为 `~/.config/altgo/altgo.toml`。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// serde 辅助模块：TOML 中为 `u64` 毫秒，Rust 侧为 `Duration`。
mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(dur: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(dur.as_millis() as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}

/// serde 辅助模块：TOML 中为 `u64` 秒，Rust 侧为 `Duration`。
mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(dur: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(dur.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

/// altgo 主配置结构体，包含所有子系统的配置。
#[derive(Debug, Default, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct Config {
    /// 按键监听配置
    pub key_listener: KeyListenerConfig,
    /// 录音配置
    pub recorder: RecorderConfig,
    /// 语音识别配置
    pub transcriber: TranscriberConfig,
    /// 文本润色配置
    pub polisher: PolisherConfig,
    /// 输出（剪切板/通知）配置
    pub output: OutputConfig,
    /// GUI 配置
    pub gui: GuiConfig,
}

/// 按键监听配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct KeyListenerConfig {
    /// 监听的按键名称（如 `Alt_L`、`Alt_R`），与 xmodmap keysym 一致
    pub key_name: String,
    /// Linux evtest 回退路径使用的 evdev 键码（由「按下以设置」捕获）；`None` 时沿用 Alt 预设的启发式映射
    pub linux_evdev_code: Option<u16>,
    /// Windows low-level keyboard hook virtual-key code captured from Settings.
    pub windows_vk: Option<i32>,
    /// 长按阈值（毫秒），超过此时间视为长按录音
    #[serde(with = "duration_ms", alias = "long_press_threshold_ms")]
    pub long_press_threshold: Duration,
    /// 双击间隔（毫秒），两次点击在此时间窗口内视为双击
    #[serde(with = "duration_ms", alias = "double_click_interval_ms")]
    pub double_click_interval: Duration,
    /// 最短按下时长（毫秒），过滤 IME 导致的瞬时分合
    #[serde(with = "duration_ms", alias = "min_press_duration_ms")]
    pub min_press_duration: Duration,
}

impl Default for KeyListenerConfig {
    fn default() -> Self {
        Self {
            key_name: "Alt_R".to_string(),
            linux_evdev_code: None,
            windows_vk: None,
            long_press_threshold: Duration::from_millis(200),
            double_click_interval: Duration::from_millis(300),
            min_press_duration: Duration::from_millis(100),
        }
    }
}

/// 录音配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct RecorderConfig {
    /// 采样率（Hz），默认 16000
    pub sample_rate: u32,
    /// 声道数，默认 1（单声道）
    pub channels: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
        }
    }
}

/// 语音识别配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct TranscriberConfig {
    /// 引擎类型：`"local"`（本地 whisper.cpp）或 `"api"`（Whisper API）
    pub engine: String,
    /// API 密钥（可通过 `ALTGO_TRANSCRIBER_API_KEY` 环境变量覆盖）
    pub api_key: String,
    /// API 基础 URL
    pub api_base_url: String,
    /// 模型名称（API 模式）或模型文件路径（本地模式）
    pub model: String,
    /// 语言代码（如 `"zh"`、`"en"`）
    pub language: String,
    /// whisper-cli 二进制文件路径（为空时自动在 PATH 中查找）
    pub whisper_path: String,
    /// 请求超时时间（秒）
    #[serde(with = "duration_secs", alias = "timeout_seconds")]
    pub timeout: Duration,
    /// Whisper API temperature（0.0 - 1.0），越低越确定性，默认 0
    pub temperature: f32,
    /// Whisper API prompt，提供上下文/词汇提示以提升识别准确率
    pub prompt: String,
    /// 本地引擎线程数；`0` 表示按 CPU 并行度自动取满（whisper 默认仅 min(4, hw)）
    pub threads: u32,
    /// 本地引擎 beam search 宽度；`<= 1` 时走贪心解码（最快），默认 0
    pub beam_size: u32,
}

impl Default for TranscriberConfig {
    fn default() -> Self {
        Self {
            engine: "local".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.openai.com".to_string(),
            model: String::new(),
            language: "zh".to_string(),
            whisper_path: String::new(),
            timeout: Duration::from_secs(30),
            temperature: 0.0,
            prompt: String::new(),
            threads: 0,
            beam_size: 0,
        }
    }
}

/// 文本润色配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct PolisherConfig {
    /// API 协议：`"openai"`（OpenAI/DeepSeek 等）或 `"anthropic"`
    pub protocol: String,
    /// API 密钥（可通过 `ALTGO_POLISHER_API_KEY` 环境变量覆盖）
    pub api_key: String,
    /// API 基础 URL（如 `https://api.openai.com`、`https://api.anthropic.com`）
    pub api_base_url: String,
    /// 模型名称（如 `"gpt-3.5-turbo"`、`"claude-sonnet-4-20250514"`）
    pub model: String,
    /// 润色级别：`"none"`、`"light"`、`"medium"`、`"heavy"`
    pub level: String,
    /// 请求超时时间（秒）
    #[serde(with = "duration_secs", alias = "timeout_seconds")]
    pub timeout: Duration,
    /// 最大生成 token 数
    pub max_tokens: u32,
    /// LLM temperature（0.0 - 2.0），默认 0.3
    pub temperature: f32,
    /// 自定义 system prompt，为空时使用内置 prompt
    pub system_prompt: String,
}

impl Default for PolisherConfig {
    fn default() -> Self {
        Self {
            protocol: "openai".to_string(),
            api_key: String::new(),
            api_base_url: String::new(),
            model: String::new(),
            level: "none".to_string(),
            timeout: Duration::from_secs(60),
            max_tokens: 1024,
            temperature: 0.3,
            system_prompt: String::new(),
        }
    }
}

/// 输出配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct OutputConfig {
    /// 是否启用桌面通知
    pub enable_notify: bool,
    /// 注入/复制时是否优先使用润色后的文本
    pub prefer_polished: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enable_notify: true,
            prefer_polished: true,
        }
    }
}

/// GUI 配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct GuiConfig {
    /// 界面语言：`"zh"` 或 `"en"`
    pub language: String,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            language: "zh".to_string(),
        }
    }
}

impl Config {
    /// 从指定路径加载配置文件。如果文件不存在，返回默认配置。
    /// 环境变量 `ALTGO_TRANSCRIBER_API_KEY` 和 `ALTGO_POLISHER_API_KEY`
    /// 会覆盖配置文件中的对应字段。
    pub fn load(path: &Path) -> Result<Self> {
        let mut cfg = if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            toml::from_str(&content).with_context(|| format!("parse {}", path.display()))?
        } else {
            Config::default()
        };

        // Environment variable overrides for API keys.
        if let Ok(key) = std::env::var("ALTGO_TRANSCRIBER_API_KEY") {
            cfg.transcriber.api_key = key;
        }
        if let Ok(key) = std::env::var("ALTGO_POLISHER_API_KEY") {
            cfg.polisher.api_key = key;
        }

        Ok(cfg)
    }

    /// Validate the loaded configuration.
    /// Call this after `load()` to check API keys are present when using API engines.
    pub fn validate(&self) -> Result<()> {
        if self.transcriber.engine == "api" && self.transcriber.api_key.trim().is_empty() {
            let config_path = Self::default_config_path();
            anyhow::bail!(
                "转写引擎设置为 'api'，但未配置 API 密钥。\n\
                 \n\
                 解决方法（任选一种）：\n\
                 1. 设置环境变量：\n\
                    export ALTGO_TRANSCRIBER_API_KEY=\"your-key\"\n\
                 2. 在配置文件中填写 api_key：\n\
                    编辑 {}，在 [transcriber] 下填写 api_key = \"your-key\"\n\
                 3. 使用本地 whisper.cpp（无需 API 密钥）：\n\
                    将 transcriber.engine 改为 \"local\"",
                config_path.display()
            );
        }
        // Only require polisher API key when polishing is actually enabled.
        if self.polisher.level != "none" && self.polisher.api_key.trim().is_empty() {
            let config_path = Self::default_config_path();
            anyhow::bail!(
                "润色功能已开启（level = \"{}\"），但未配置 API 密钥。\n\
                 \n\
                 请在配置文件中填写 [polisher] 段：\n\
                   api_key = \"your-key\"\n\
                   api_base_url = \"https://your-provider.com\"\n\
                   model = \"your-model-name\"\n\
                   protocol = \"openai\"  # 或 \"anthropic\"\n\
                 \n\
                 或通过环境变量设置密钥：\n\
                   export ALTGO_POLISHER_API_KEY=\"your-key\"\n\
                 \n\
                 如果不需要润色，将 level 改为 \"none\" 即可跳过此检查。\n\
                 配置文件路径：{}",
                self.polisher.level,
                config_path.display()
            );
        }
        Ok(())
    }

    /// 将配置保存到指定路径。
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config to TOML")?;

        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create directory {}", parent.display()))?;
        }

        std::fs::write(path, content)
            .with_context(|| format!("write config to {}", path.display()))?;

        // Restrict file permissions to owner-only (protect API keys at rest).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }

        tracing::info!(path = %path.display(), "config saved");
        Ok(())
    }

    /// 返回默认配置文件路径（`~/.config/altgo/altgo.toml`）。
    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("altgo")
            .join("altgo.toml")
    }
}

// ---------------------------------------------------------------------------
// ConfigPatch — partial update for Config
// ---------------------------------------------------------------------------

/// 三态反序列化：JSON 字段缺失 = 不修改；`null` = 清除；数值 = 设置。
///
/// 紧靠 `KeyListenerConfig.linux_evdev_code` 字段定义。
fn deserialize_opt_patch_u16<'de, D>(deserializer: D) -> Result<Option<Option<u16>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let u = n
                .as_u64()
                .ok_or_else(|| serde::de::Error::custom("linux_evdev_code: expected number"))?;
            if u > u16::MAX as u64 {
                return Err(serde::de::Error::custom("linux_evdev_code out of range"));
            }
            Ok(Some(Some(u as u16)))
        }
        _ => Err(serde::de::Error::custom(
            "linux_evdev_code: expected null or number",
        )),
    }
}

/// 三态反序列化：JSON 字段缺失 = 不修改；`null` = 清除；数值 = 设置。
///
/// 紧靠 `KeyListenerConfig.windows_vk` 字段定义。
fn deserialize_opt_patch_i32<'de, D>(deserializer: D) -> Result<Option<Option<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let i = n
                .as_i64()
                .ok_or_else(|| serde::de::Error::custom("windows_vk: expected number"))?;
            if i < i32::MIN as i64 || i > i32::MAX as i64 {
                return Err(serde::de::Error::custom("windows_vk out of range"));
            }
            Ok(Some(Some(i as i32)))
        }
        _ => Err(serde::de::Error::custom(
            "windows_vk: expected null or number",
        )),
    }
}

fn apply_nested_opt_u16(target: &mut Option<u16>, patch: Option<Option<u16>>) {
    match patch {
        None => {}
        Some(None) => *target = None,
        Some(Some(v)) => *target = Some(v),
    }
}

fn apply_nested_opt_i32(target: &mut Option<i32>, patch: Option<Option<i32>>) {
    match patch {
        None => {}
        Some(None) => *target = None,
        Some(Some(v)) => *target = Some(v),
    }
}

/// Partial update applied to the in-memory config. All fields are optional;
/// absent fields are left unchanged.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    pub key_name: Option<String>,
    /// `None` = field absent (no change); `Some(None)` = JSON `null` (clear);
    /// `Some(Some(v))` = set to v.
    #[serde(default, deserialize_with = "deserialize_opt_patch_u16")]
    pub linux_evdev_code: Option<Option<u16>>,
    /// `None` = field absent (no change); `Some(None)` = JSON `null` (clear);
    /// `Some(Some(v))` = set to v.
    #[serde(default, deserialize_with = "deserialize_opt_patch_i32")]
    pub windows_vk: Option<Option<i32>>,
    pub language: Option<String>,
    pub engine: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub polish_level: Option<String>,
    pub polish_model: Option<String>,
    pub polish_api_key: Option<String>,
    pub polish_api_base_url: Option<String>,
    pub gui_language: Option<String>,
}

impl ConfigPatch {
    /// 将 patch 中的 `Some` 字段写入 `cfg`。
    pub fn apply_to_config(&self, cfg: &mut Config) {
        if let Some(ref v) = self.key_name {
            cfg.key_listener.key_name = v.clone();
        }
        apply_nested_opt_u16(
            &mut cfg.key_listener.linux_evdev_code,
            self.linux_evdev_code,
        );
        apply_nested_opt_i32(&mut cfg.key_listener.windows_vk, self.windows_vk);
        if let Some(ref v) = self.language {
            cfg.transcriber.language = v.clone();
        }
        if let Some(ref v) = self.engine {
            cfg.transcriber.engine = v.clone();
        }
        if let Some(ref v) = self.model {
            cfg.transcriber.model = v.clone();
        }
        if let Some(ref v) = self.api_key {
            cfg.transcriber.api_key = v.clone();
        }
        if let Some(ref v) = self.api_base_url {
            cfg.transcriber.api_base_url = v.clone();
        }
        if let Some(ref v) = self.polish_level {
            cfg.polisher.level = v.clone();
        }
        if let Some(ref v) = self.polish_model {
            cfg.polisher.model = v.clone();
        }
        if let Some(ref v) = self.polish_api_key {
            cfg.polisher.api_key = v.clone();
        }
        if let Some(ref v) = self.polish_api_base_url {
            cfg.polisher.api_base_url = v.clone();
        }
        if let Some(ref v) = self.gui_language {
            cfg.gui.language = v.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.key_listener.key_name, "Alt_R");
        assert!(cfg.key_listener.linux_evdev_code.is_none());
        assert!(cfg.key_listener.windows_vk.is_none());
        assert_eq!(
            cfg.key_listener.long_press_threshold,
            Duration::from_millis(200)
        );
        assert_eq!(
            cfg.key_listener.min_press_duration,
            Duration::from_millis(100)
        );
        assert_eq!(cfg.recorder.sample_rate, 16000);
        assert_eq!(cfg.transcriber.engine, "local");
        assert_eq!(cfg.transcriber.temperature, 0.0);
        assert_eq!(cfg.polisher.level, "none");
        assert_eq!(cfg.polisher.temperature, 0.3);
        assert!(cfg.output.enable_notify);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let cfg = Config::load(Path::new("/nonexistent/altgo.toml")).unwrap();
        assert_eq!(cfg.recorder.sample_rate, 16000);
    }

    #[test]
    fn test_load_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"
[key_listener]
key_name = "Alt_R"

[recorder]
sample_rate = 48000

[transcriber]
engine = "api"
language = "en"

[polisher]
level = "heavy"

[output]
enable_notify = false
"#
        )
        .unwrap();

        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.key_listener.key_name, "Alt_R");
        assert_eq!(cfg.recorder.sample_rate, 48000);
        assert_eq!(cfg.transcriber.engine, "api");
        assert_eq!(cfg.transcriber.language, "en");
        assert_eq!(cfg.polisher.level, "heavy");
        assert!(!cfg.output.enable_notify);
    }

    #[test]
    fn test_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        std::fs::write(&path, "this is not valid [[[").unwrap();
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn test_windows_vk_round_trips_through_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        let mut cfg = Config::default();
        cfg.key_listener.key_name = "Right Alt".to_string();
        cfg.key_listener.windows_vk = Some(0xA5);

        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();

        assert_eq!(loaded.key_listener.key_name, "Right Alt");
        assert_eq!(loaded.key_listener.windows_vk, Some(0xA5));
    }

    #[test]
    #[allow(deprecated)]
    fn test_env_override() {
        // std::env::set_var is deprecated in Rust 1.94+ due to thread-safety concerns.
        // Acceptable here: single-threaded test, no concurrent access.
        std::env::set_var("ALTGO_TRANSCRIBER_API_KEY", "test-trans-key");
        std::env::set_var("ALTGO_POLISHER_API_KEY", "test-polish-key");

        let cfg = Config::load(Path::new("/nonexistent.toml")).unwrap();
        assert_eq!(cfg.transcriber.api_key, "test-trans-key");
        assert_eq!(cfg.polisher.api_key, "test-polish-key");

        std::env::remove_var("ALTGO_TRANSCRIBER_API_KEY");
        std::env::remove_var("ALTGO_POLISHER_API_KEY");
    }

    #[test]
    fn test_duration_fields() {
        let cfg = Config::default();
        assert_eq!(cfg.transcriber.timeout, Duration::from_secs(30));
        assert_eq!(cfg.polisher.timeout, Duration::from_secs(60));
        assert_eq!(
            cfg.key_listener.long_press_threshold,
            Duration::from_millis(200)
        );
        assert_eq!(
            cfg.key_listener.double_click_interval,
            Duration::from_millis(300)
        );
        assert_eq!(
            cfg.key_listener.min_press_duration,
            Duration::from_millis(100)
        );
    }

    #[test]
    fn test_default_config_path() {
        let path = Config::default_config_path();
        assert!(path.to_string_lossy().contains("altgo"));
        assert!(path.to_string_lossy().ends_with("altgo.toml"));
    }

    #[test]
    fn test_validate_missing_api_key() {
        let mut cfg = Config::default();
        cfg.transcriber.engine = "api".to_string();
        cfg.transcriber.api_key = String::new();
        cfg.polisher.api_key = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_success_with_api_key() {
        let mut cfg = Config::default();
        cfg.transcriber.engine = "api".to_string();
        cfg.transcriber.api_key = "test-key".to_string();
        cfg.polisher.api_key = "polish-key".to_string();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_whitespace_api_key_fails() {
        let mut cfg = Config::default();
        cfg.transcriber.engine = "api".to_string();
        cfg.transcriber.api_key = "   ".to_string();
        cfg.polisher.api_key = "valid-key".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_local_mode_no_polisher_key() {
        // Local transcriber with polishing disabled should not require API keys.
        let mut cfg = Config::default();
        cfg.transcriber.engine = "local".to_string();
        cfg.transcriber.api_key = String::new();
        cfg.polisher.level = "none".to_string();
        cfg.polisher.api_key = String::new();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_polisher_requires_key_when_enabled() {
        // When polisher level != "none", API key is required.
        let mut cfg = Config::default();
        cfg.transcriber.engine = "local".to_string();
        cfg.transcriber.api_key = String::new();
        cfg.polisher.level = "medium".to_string();
        cfg.polisher.api_key = String::new();
        assert!(cfg.validate().is_err());
    }

    // -- ConfigPatch tests ---------------------------------------------------

    #[test]
    fn evdev_json_null_clears() {
        let j = r#"{"linuxEvdevCode":null}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert_eq!(p.linux_evdev_code, Some(None));
    }

    #[test]
    fn evdev_missing_field_means_no_patch() {
        let j = r#"{}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert!(p.linux_evdev_code.is_none());
    }

    #[test]
    fn evdev_number_sets() {
        let j = r#"{"linuxEvdevCode":100}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert_eq!(p.linux_evdev_code, Some(Some(100)));
    }

    #[test]
    fn windows_vk_json_null_clears() {
        let j = r#"{"windowsVk":null}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert_eq!(p.windows_vk, Some(None));
    }

    #[test]
    fn windows_vk_missing_field_means_no_patch() {
        let j = r#"{}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert!(p.windows_vk.is_none());
    }

    #[test]
    fn windows_vk_number_sets() {
        let j = r#"{"windowsVk":165}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert_eq!(p.windows_vk, Some(Some(165)));
    }

    #[test]
    fn patch_apply_to_config_updates_selected_fields() {
        let mut cfg = Config::default();
        let patch: ConfigPatch =
            serde_json::from_str(r#"{"keyName":"space","language":"en","polishLevel":"heavy"}"#)
                .unwrap();
        patch.apply_to_config(&mut cfg);
        assert_eq!(cfg.key_listener.key_name, "space");
        assert_eq!(cfg.transcriber.language, "en");
        assert_eq!(cfg.polisher.level, "heavy");
        // Unchanged fields keep defaults
        assert_eq!(cfg.transcriber.engine, "local");
    }

    #[test]
    fn patch_apply_evdev_null_clears() {
        let mut cfg = Config::default();
        cfg.key_listener.linux_evdev_code = Some(56);
        let patch: ConfigPatch = serde_json::from_str(r#"{"linuxEvdevCode":null}"#).unwrap();
        patch.apply_to_config(&mut cfg);
        assert!(cfg.key_listener.linux_evdev_code.is_none());
    }

    #[test]
    fn patch_round_trip_through_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");

        let mut cfg = Config::default();
        let patch: ConfigPatch =
            serde_json::from_str(r#"{"keyName":"F1","language":"en"}"#).unwrap();
        patch.apply_to_config(&mut cfg);
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.key_listener.key_name, "F1");
        assert_eq!(loaded.transcriber.language, "en");
    }
}
