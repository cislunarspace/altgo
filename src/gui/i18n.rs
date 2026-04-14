//! Simple i18n module — static key/value translations for zh/en.
//!
//! No external dependencies. Add new keys to both `t_zh` and `t_en`.

/// All known translation keys — must be kept in sync with `t_zh` and `t_en`.
#[allow(dead_code)]
const KEYS: &[&str] = &[
    // Menu bar
    "menu.settings",
    "menu.file",
    "menu.exit",
    "menu.help",
    "menu.about",
    // Title
    "title.subtitle",
    // About dialog
    "about.text",
    // Status bar
    "status.idle",
    "status.recording",
    "status.processing",
    "status.done",
    // Main content
    "main.idle",
    "main.recording",
    "main.processing",
    "main.hint",
    "main.result_label",
    "main.copied",
    // Settings panel
    "settings.title",
    "settings.recording",
    "settings.key_name",
    "settings.transcription",
    "settings.engine",
    "settings.engine_api",
    "settings.engine_local",
    "settings.language",
    "settings.model",
    "settings.model_path",
    "settings.api_key",
    "settings.api_url",
    "settings.polishing",
    "settings.polish_level",
    "settings.polish_none",
    "settings.polish_light",
    "settings.polish_medium",
    "settings.polish_heavy",
    "settings.save",
    "settings.cancel",
    "settings.restart_hint",
    "settings.gui_language",
    "settings.lang_zh",
    "settings.lang_en",
    "settings.error_invalid_polish_level",
    "settings.error_invalid_engine",
    // Tray
    "tray.show",
    "tray.settings",
    "tray.exit",
    "tray.tooltip",
    // Window title
    "window.title",
    // Notifications
    "notify.polish_failed",
    "notify.processing",
];

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
pub enum Lang {
    #[default]
    Zh,
    En,
}

impl Lang {
    /// Parse from config string (e.g. "zh", "en").
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_lowercase().as_str() {
            "en" | "english" => Lang::En,
            _ => Lang::Zh,
        }
    }

    /// Serialize back to config string.
    pub fn code(&self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }
}

/// Look up a translated string by key.
pub fn t(key: &'static str, lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => t_zh(key),
        Lang::En => t_en(key),
    }
}

fn t_zh(key: &'static str) -> &'static str {
    match key {
        // Menu bar
        "menu.settings" => "设置",
        "menu.file" => "文件",
        "menu.exit" => "退出",
        "menu.help" => "帮助",
        "menu.about" => "关于 altgo",

        // Title
        "title.subtitle" => "语音转文字",

        // About dialog
        "about.text" => "altgo v0.1.0\n无需打字，言出法随\n按住 Alt 键说话，松开自动转写",

        // Status bar
        "status.idle" => "等待说话",
        "status.recording" => "正在录音...",
        "status.processing" => "正在转写...",
        "status.done" => "转写完成，已复制到剪贴板",

        // Main content
        "main.idle" => "按住 右 Alt 键说话",
        "main.recording" => "正在录音...",
        "main.processing" => "正在处理...",
        "main.hint" => "松开后自动转写并复制到剪贴板",
        "main.result_label" => "转写结果",
        "main.copied" => "已复制到剪贴板 ✓",

        // Settings panel
        "settings.title" => "设置",
        "settings.recording" => "录音设置",
        "settings.key_name" => "按键名称:",
        "settings.transcription" => "转写设置",
        "settings.engine" => "引擎:",
        "settings.engine_api" => "API (OpenAI兼容)",
        "settings.engine_local" => "本地 (whisper.cpp)",
        "settings.language" => "语言:",
        "settings.model" => "模型:",
        "settings.model_path" => "模型路径:",
        "settings.api_key" => "API Key:",
        "settings.api_url" => "API URL:",
        "settings.polishing" => "润色设置",
        "settings.polish_level" => "润色级别:",
        "settings.polish_none" => "关闭",
        "settings.polish_light" => "轻度",
        "settings.polish_medium" => "中度",
        "settings.polish_heavy" => "重度",
        "settings.save" => "保存",
        "settings.cancel" => "取消",
        "settings.restart_hint" => "提示: 部分设置需要重启应用后生效",
        "settings.gui_language" => "界面语言:",
        "settings.lang_zh" => "中文",
        "settings.lang_en" => "English",
        "settings.error_invalid_polish_level" => "无效的润色级别",
        "settings.error_invalid_engine" => "无效的引擎",

        // Tray
        "tray.show" => "显示窗口",
        "tray.settings" => "设置",
        "tray.exit" => "退出",
        "tray.tooltip" => "altgo — 按住 Alt 说话",

        // Window title
        "window.title" => "altgo — 语音转文字",

        // Notifications
        "notify.polish_failed" => "润色失败，已使用原始文本",
        "notify.processing" => "正在处理语音...",

        _ => key,
    }
}

fn t_en(key: &'static str) -> &'static str {
    match key {
        // Menu bar
        "menu.settings" => "Settings",
        "menu.file" => "File",
        "menu.exit" => "Exit",
        "menu.help" => "Help",
        "menu.about" => "About altgo",

        // Title
        "title.subtitle" => "Voice to Text",

        // About dialog
        "about.text" => "altgo v0.1.0\nSpeak naturally, type automatically\nHold Alt to talk, release to transcribe",

        // Status bar
        "status.idle" => "Ready",
        "status.recording" => "Recording...",
        "status.processing" => "Transcribing...",
        "status.done" => "Done — copied to clipboard",

        // Main content
        "main.idle" => "Hold Right Alt to speak",
        "main.recording" => "Recording...",
        "main.processing" => "Processing...",
        "main.hint" => "Release to transcribe and copy to clipboard",
        "main.result_label" => "Transcription",
        "main.copied" => "Copied to clipboard ✓",

        // Settings panel
        "settings.title" => "Settings",
        "settings.recording" => "Recording",
        "settings.key_name" => "Key name:",
        "settings.transcription" => "Transcription",
        "settings.engine" => "Engine:",
        "settings.engine_api" => "API (OpenAI compatible)",
        "settings.engine_local" => "Local (whisper.cpp)",
        "settings.language" => "Language:",
        "settings.model" => "Model:",
        "settings.model_path" => "Model path:",
        "settings.api_key" => "API Key:",
        "settings.api_url" => "API URL:",
        "settings.polishing" => "Polishing",
        "settings.polish_level" => "Polish level:",
        "settings.polish_none" => "Off",
        "settings.polish_light" => "Light",
        "settings.polish_medium" => "Medium",
        "settings.polish_heavy" => "Heavy",
        "settings.save" => "Save",
        "settings.cancel" => "Cancel",
        "settings.restart_hint" => "Note: Some settings require restart to take effect",
        "settings.gui_language" => "UI Language:",
        "settings.lang_zh" => "中文",
        "settings.lang_en" => "English",
        "settings.error_invalid_polish_level" => "Invalid polish level",
        "settings.error_invalid_engine" => "Invalid engine",

        // Tray
        "tray.show" => "Show Window",
        "tray.settings" => "Settings",
        "tray.exit" => "Exit",
        "tray.tooltip" => "altgo — Hold Alt to speak",

        // Window title
        "window.title" => "altgo — Voice to Text",

        // Notifications
        "notify.polish_failed" => "Polish failed, using raw text",
        "notify.processing" => "Processing speech...",

        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zh_and_en_keys_match() {
        for key in KEYS {
            let zh = t_zh(key);
            let en = t_en(key);
            assert_ne!(zh, *key, "Chinese translation missing for key: {key}");
            assert_ne!(en, *key, "English translation missing for key: {key}");
        }
    }
}
