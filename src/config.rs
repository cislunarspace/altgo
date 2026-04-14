//! 配置加载模块。
//!
//! 从 TOML 文件加载 altgo 配置，所有字段均通过 `serde(default)` 提供默认值，
//! 因此部分配置文件也可以正常工作。
//!
//! API 密钥支持通过环境变量覆盖：
//! - `ALTGO_TRANSCRIBER_API_KEY` — 覆盖语音识别 API 密钥
//! - `ALTGO_POLISHER_API_KEY` — 覆盖文本润色 API 密钥
//!
//! 默认配置路径为 `~/.config/altgo/altgo.toml`（Linux/macOS）
//! 或 `%APPDATA%/altgo/altgo.toml`（Windows）。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    /// 日志配置
    pub logging: LoggingConfig,
    /// GUI 配置
    pub gui: GuiConfig,
}

/// 按键监听配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct KeyListenerConfig {
    /// 监听的按键名称（如 `ISO_Level3_Shift`、`Alt_R`）
    pub key_name: String,
    /// 长按阈值（毫秒），超过此时间视为长按录音
    pub long_press_threshold_ms: u64,
    /// 双击间隔（毫秒），两次点击在此时间窗口内视为双击
    pub double_click_interval_ms: u64,
    /// 防抖窗口（毫秒），过滤 Windows 中文输入法导致的按键抖动
    pub debounce_window_ms: u64,
}

impl KeyListenerConfig {
    /// 将长按阈值转换为 `Duration`。
    pub fn long_press_threshold(&self) -> Duration {
        Duration::from_millis(self.long_press_threshold_ms)
    }

    /// 将双击间隔转换为 `Duration`。
    pub fn double_click_interval(&self) -> Duration {
        Duration::from_millis(self.double_click_interval_ms)
    }

    /// 将防抖窗口转换为 `Duration`。
    pub fn debounce_window(&self) -> Duration {
        Duration::from_millis(self.debounce_window_ms)
    }
}

impl Default for KeyListenerConfig {
    fn default() -> Self {
        Self {
            key_name: "ISO_Level3_Shift".to_string(),
            long_press_threshold_ms: 300,
            double_click_interval_ms: 300,
            debounce_window_ms: 100,
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
    /// 请求超时时间（秒）
    pub timeout_seconds: u64,
}

impl TranscriberConfig {
    /// 将超时秒数转换为 `Duration`。
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

impl Default for TranscriberConfig {
    fn default() -> Self {
        Self {
            engine: "local".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.openai.com".to_string(),
            model: String::new(),
            language: "zh".to_string(),
            timeout_seconds: 30,
        }
    }
}

/// 文本润色配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct PolisherConfig {
    /// 引擎类型（目前仅 `"openai"` 兼容接口）
    pub engine: String,
    /// API 密钥（可通过 `ALTGO_POLISHER_API_KEY` 环境变量覆盖）
    pub api_key: String,
    /// API 基础 URL
    pub api_base_url: String,
    /// 模型名称（如 `"gpt-3.5-turbo"`、`"deepseek-chat"`）
    pub model: String,
    /// 润色级别：`"none"`、`"light"`、`"medium"`、`"heavy"`
    pub level: String,
    /// 请求超时时间（秒）
    pub timeout_seconds: u64,
    /// 最大生成 token 数
    pub max_tokens: u32,
}

impl PolisherConfig {
    /// 将超时秒数转换为 `Duration`。
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

impl Default for PolisherConfig {
    fn default() -> Self {
        Self {
            engine: "openai".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.openai.com".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            level: "medium".to_string(),
            timeout_seconds: 60,
            max_tokens: 1024,
        }
    }
}

/// 输出配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct OutputConfig {
    /// 是否启用桌面通知
    pub enable_notify: bool,
    /// 通知显示时长（毫秒）
    pub notify_timeout_ms: u64,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enable_notify: true,
            notify_timeout_ms: 3000,
        }
    }
}

/// 日志配置。
#[derive(Debug, Deserialize, Clone, serde::Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// 日志级别（如 `"info"`、`"debug"`、`"warn"`）
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.key_listener.key_name, "ISO_Level3_Shift");
        assert_eq!(cfg.key_listener.long_press_threshold_ms, 300);
        assert_eq!(cfg.key_listener.debounce_window_ms, 100);
        assert_eq!(cfg.recorder.sample_rate, 16000);
        assert_eq!(cfg.transcriber.engine, "local");
        assert_eq!(cfg.polisher.level, "medium");
        assert!(cfg.output.enable_notify);
        assert_eq!(cfg.logging.level, "info");
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

[logging]
level = "debug"
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
        assert_eq!(cfg.logging.level, "debug");
    }

    #[test]
    fn test_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        std::fs::write(&path, "this is not valid [[[").unwrap();
        assert!(Config::load(&path).is_err());
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
    fn test_timeout_helpers() {
        let cfg = Config::default();
        assert_eq!(cfg.transcriber.timeout(), Duration::from_secs(30));
        assert_eq!(cfg.polisher.timeout(), Duration::from_secs(60));
        assert_eq!(
            cfg.key_listener.long_press_threshold(),
            Duration::from_millis(300)
        );
        assert_eq!(
            cfg.key_listener.double_click_interval(),
            Duration::from_millis(300)
        );
        assert_eq!(
            cfg.key_listener.debounce_window(),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn test_default_config_path() {
        let path = Config::default_config_path();
        assert!(path.to_string_lossy().contains("altgo"));
        assert!(path.to_string_lossy().ends_with("altgo.toml"));
    }
}
