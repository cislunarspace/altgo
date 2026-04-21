import { useState, useEffect } from "react";

const LANG_KEY = "altgo-lang";

const translations: Record<string, Record<string, string>> = {
  zh: {
    "title.subtitle": "语音转文字",
    "nav.home": "首页",
    "nav.settings": "设置",
    "status.idle": "等待说话",
    "status.recording": "正在录音...",
    "status.processing": "正在转写...",
    "status.done": "转写完成",
    "main.hint": "按住 右Alt 键说话，松开自动转写",
    "main.hint_clipboard": "转写完成后文本会写入剪贴板，可直接粘贴使用。",
    "main.copy": "复制到剪贴板",
    "main.result_label": "转写结果",
    "main.copied": "已复制到剪贴板 ✓",
    "main.error_title": "错误",
    "main.key_backend_evtest": "监听方式：本机输入设备（evdev），适用于 Wayland",
    "main.key_backend_xinput": "监听方式：X11 xinput",
    "settings.title": "设置",
    "settings.loading": "加载中...",
    "settings.saved": "设置已保存",
    "settings.saving": "保存中...",
    "settings.appearance": "外观",
    "settings.appearance_lead": "选择浅色或深色界面；「跟随系统」会随操作系统明暗模式自动切换。",
    "settings.theme": "配色",
    "settings.theme_system": "跟随系统",
    "settings.theme_light": "浅色",
    "settings.theme_dark": "深色",
    "settings.gui_language": "界面语言",
    "settings.recording": "录音设置",
    "settings.key_name": "触发键",
    "settings.transcription": "转写设置",
    "settings.engine": "引擎",
    "settings.engine_api": "API (OpenAI兼容)",
    "settings.engine_local": "本地 (whisper.cpp)",
    "settings.language": "语言",
    "settings.model": "模型",
    "settings.active_model": "当前模型",
    "settings.active_model_placeholder": "留空自动选择，或输入模型名（如 base）",
    "settings.active_model_hint": "输入模型名称（如 tiny, base, small, medium, large）或留空自动选择已下载的第一个模型。",
    "settings.api_url": "API URL",
    "settings.api_key": "API Key",
    "settings.polishing": "润色设置",
    "settings.polish_level": "润色级别",
    "settings.polish_none": "关闭",
    "settings.polish_light": "轻度",
    "settings.polish_medium": "中度",
    "settings.polish_heavy": "重度",
    "settings.polish_level_hint":
      "转写完成后用大模型整理标点、口语赘词等；级别越高改写越多，通常更耗时、API 花费更高。关闭则仅保留 Whisper 原文。",
    "settings.model_management": "模型管理",
    "settings.current_model": "当前模型",
    "settings.download_model": "下载模型",
    "settings.delete_model": "删除模型",
    "settings.model_downloaded": "已下载",
    "settings.model_not_downloaded": "未下载",
    "settings.confirm_delete": "确认删除？",
    "settings.no_model_configured": "未配置",
    "settings.about": "关于",
    "settings.about_tagline": "按住快捷键说话，松手转写并润色，把口述变成可直接使用的文字。",
    "settings.version": "版本",
    "settings.check_updates": "检查更新",
    "settings.save": "保存",
    "settings.restart_hint": "保存后会自动重载语音管道（按键监听与转写配置），一般无需重启应用。",
    "settings.models": "模型管理",
    "settings.model_download": "下载",
    "settings.model_delete": "删除",
    "settings.transcription_lead": "语音转文字依赖本页配置。本地模式需先下载并选中一个模型；保存后立即生效。",
    "settings.recording_lead": "默认使用键盘右侧 Alt。若无效可改为左 Alt。",
    "settings.readiness_api": "云端 API 模式",
    "settings.readiness_api_desc": "请填写有效的 API 地址与密钥后保存。",
    "settings.readiness_local_ok": "本地转写已就绪",
    "settings.readiness_local_need": "请完成本地模型设置",
    "settings.readiness_local_desc": "从下方选择一个模型，点击「下载并启用」；若已下载，点「使用此模型」并保存。",
    "settings.readiness_path_missing": "当前填写的模型名或路径在磁盘上找不到，请重新选择或下载。",
    "settings.key_preset_right_alt": "右 Alt（推荐，ISO_Level3_Shift）",
    "settings.key_preset_left_alt": "左 Alt（Alt_L）",
    "settings.key_preset_alt_r": "Alt_R",
    "settings.key_custom": "自定义键名…",
    "settings.key_custom_value": "自定义 keysym",
    "settings.capture_activation": "按下以设置",
    "settings.capture_activation_short": "按下以设置快捷键",
    "settings.capture_waiting": "请按键…（约 12 秒内）",
    "settings.capture_activation_lead":
      "点击后请按下要作为激活键的按键；成功后会自动保存并重载监听。失败时可再试或手动填写 keysym。",
    "settings.in_use": "当前",
    "settings.use_model": "使用此模型",
    "settings.current": "已选用",
    "settings.download_and_use": "下载并启用",
    "settings.advanced_model_path": "高级：自定义路径或模型名",
    "settings.custom_path": "模型名或文件路径",
    "settings.custom_path_placeholder": "例如 base 或 /path/to/ggml-base.bin",
    "settings.custom_path_hint": "可填内置名称（tiny、base…）或本机 .bin 文件的完整路径。",
    "overlay.recording": "录音中...",
    "overlay.processing": "处理中...",
    "overlay.copy": "复制",
    "overlay.copied": "已复制",
  },
  en: {
    "title.subtitle": "Voice to Text",
    "nav.home": "Home",
    "nav.settings": "Settings",
    "status.idle": "Ready",
    "status.recording": "Recording...",
    "status.processing": "Transcribing...",
    "status.done": "Done",
    "main.hint": "Hold Right Alt to speak, release to transcribe",
    "main.hint_clipboard": "When transcription finishes, text is placed on the clipboard—paste it anywhere.",
    "main.copy": "Copy to clipboard",
    "main.result_label": "Transcription",
    "main.copied": "Copied to clipboard ✓",
    "main.error_title": "Error",
    "main.key_backend_evtest": "Input: evdev (recommended on Wayland)",
    "main.key_backend_xinput": "Input: X11 xinput",
    "settings.title": "Settings",
    "settings.loading": "Loading...",
    "settings.saved": "Settings saved",
    "settings.saving": "Saving...",
    "settings.appearance": "Appearance",
    "settings.appearance_lead": "Choose light or dark UI. \"Match system\" follows your OS light/dark setting.",
    "settings.theme": "Color theme",
    "settings.theme_system": "Match system",
    "settings.theme_light": "Light",
    "settings.theme_dark": "Dark",
    "settings.gui_language": "UI Language",
    "settings.recording": "Recording",
    "settings.key_name": "Trigger Key",
    "settings.transcription": "Transcription",
    "settings.engine": "Engine",
    "settings.engine_api": "API (OpenAI compatible)",
    "settings.engine_local": "Local (whisper.cpp)",
    "settings.language": "Language",
    "settings.model": "Model",
    "settings.active_model": "Active Model",
    "settings.active_model_placeholder": "Leave empty for auto, or enter model name (e.g. base)",
    "settings.active_model_hint": "Enter a model name (tiny, base, small, medium, large) or leave empty to auto-select the first downloaded model.",
    "settings.api_url": "API URL",
    "settings.api_key": "API Key",
    "settings.polishing": "Polishing",
    "settings.polish_level": "Polish level",
    "settings.polish_none": "Off",
    "settings.polish_light": "Light",
    "settings.polish_medium": "Medium",
    "settings.polish_heavy": "Heavy",
    "settings.polish_level_hint":
      "After transcription, the LLM cleans punctuation and filler; higher levels rewrite more, usually taking longer and costing more. Off keeps the raw Whisper text.",
    "settings.model_management": "Model Management",
    "settings.current_model": "Current Model",
    "settings.download_model": "Download Model",
    "settings.delete_model": "Delete Model",
    "settings.model_downloaded": "Downloaded",
    "settings.model_not_downloaded": "Not Downloaded",
    "settings.confirm_delete": "Confirm Delete?",
    "settings.no_model_configured": "No Model Configured",
    "settings.about": "About",
    "settings.about_tagline": "Hold the shortcut to speak, release to transcribe and polish—turn speech into usable text.",
    "settings.version": "Version",
    "settings.check_updates": "Check for Updates",
    "settings.save": "Save",
    "settings.restart_hint": "After saving, the voice pipeline reloads automatically; you usually do not need to restart the app.",
    "settings.models": "Model Management",
    "settings.model_download": "Download",
    "settings.model_delete": "Delete",
    "settings.transcription_lead": "Transcription uses the settings below. For local mode, download and select a model; changes apply after you save.",
    "settings.recording_lead": "Default is Right Alt. Switch to Left Alt if your layout requires it.",
    "settings.readiness_api": "Cloud API mode",
    "settings.readiness_api_desc": "Enter a valid API base URL and key, then save.",
    "settings.readiness_local_ok": "Local transcription ready",
    "settings.readiness_local_need": "Finish local model setup",
    "settings.readiness_local_desc": "Pick a model below and click Download & enable; if already downloaded, click Use this model and save.",
    "settings.readiness_path_missing": "The configured model name or path was not found on disk.",
    "settings.key_preset_right_alt": "Right Alt (recommended, ISO_Level3_Shift)",
    "settings.key_preset_left_alt": "Left Alt (Alt_L)",
    "settings.key_preset_alt_r": "Alt_R",
    "settings.key_custom": "Custom keysym…",
    "settings.key_custom_value": "Custom keysym",
    "settings.capture_activation": "Press to set",
    "settings.capture_activation_short": "Set key by pressing",
    "settings.capture_waiting": "Press a key… (within ~12s)",
    "settings.capture_activation_lead":
      "Click, then press the key you want to use. Settings save and the listener reloads automatically. On failure, try again or enter a keysym manually.",
    "settings.in_use": "Active",
    "settings.use_model": "Use this model",
    "settings.current": "Selected",
    "settings.download_and_use": "Download & enable",
    "settings.advanced_model_path": "Advanced: custom path or name",
    "settings.custom_path": "Model name or file path",
    "settings.custom_path_placeholder": "e.g. base or /path/to/ggml-base.bin",
    "settings.custom_path_hint": "Use a built-in name (tiny, base, …) or a full path to a .bin file.",
    "overlay.recording": "Recording...",
    "overlay.processing": "Processing...",
    "overlay.copy": "Copy",
    "overlay.copied": "Copied",
  },
};

export function translateStatic(lang: string, key: string): string {
  return translations[lang]?.[key] ?? translations["zh"]?.[key] ?? key;
}

export function useTranslation() {
  const [lang, setLangState] = useState<string>(
    () => localStorage.getItem(LANG_KEY) || "zh"
  );

  const setLang = (code: string) => {
    setLangState(code);
    localStorage.setItem(LANG_KEY, code);
  };

  const t = (key: string): string => {
    return translateStatic(lang, key);
  };

  return { t, lang, setLang };
}

/** For the overlay window: follows `altgo-lang` via storage events from the main window. */
export function useOverlayTranslation() {
  const [lang, setLang] = useState<string>(
    () => localStorage.getItem(LANG_KEY) || "zh"
  );

  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === LANG_KEY && e.newValue) {
        setLang(e.newValue);
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const t = (key: string) => translateStatic(lang, key);
  return { t, lang };
}
