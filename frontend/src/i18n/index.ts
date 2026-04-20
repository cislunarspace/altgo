import { useState } from "react";

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
    "main.result_label": "转写结果",
    "main.copied": "已复制到剪贴板 ✓",
    "main.error_title": "错误",
    "settings.title": "设置",
    "settings.loading": "加载中...",
    "settings.saved": "设置已保存",
    "settings.saving": "保存中...",
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
    "settings.save": "保存",
    "settings.restart_hint": "提示：保存后语音转写服务将自动重启",
    "settings.models": "模型管理",
    "settings.model_download": "下载",
    "settings.model_delete": "删除",
    "overlay.recording": "录音中...",
    "overlay.processing": "处理中...",
    "overlay.copy": "复制",
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
    "main.result_label": "Transcription",
    "main.copied": "Copied to clipboard ✓",
    "main.error_title": "Error",
    "settings.title": "Settings",
    "settings.loading": "Loading...",
    "settings.saved": "Settings saved",
    "settings.saving": "Saving...",
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
    "settings.save": "Save",
    "settings.restart_hint": "Note: the transcription service will auto-restart after saving",
    "settings.models": "Model Management",
    "settings.model_download": "Download",
    "settings.model_delete": "Delete",
    "overlay.recording": "Recording...",
    "overlay.processing": "Processing...",
    "overlay.copy": "Copy",
  },
};

export function useTranslation() {
  const [lang, setLangState] = useState<string>(
    () => localStorage.getItem("altgo-lang") || "zh"
  );

  const setLang = (code: string) => {
    setLangState(code);
    localStorage.setItem("altgo-lang", code);
  };

  const t = (key: string): string => {
    return translations[lang]?.[key] ?? translations["zh"]?.[key] ?? key;
  };

  return { t, lang, setLang };
}
