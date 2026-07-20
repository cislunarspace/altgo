import { useState, useEffect } from "react";

const LANG_KEY = "altgo-lang";

const translations: Record<string, Record<string, string>> = {
  zh: {
    "title.subtitle": "语音转文字",
    "nav.home": "首页",
    "nav.history": "历史",
    "nav.settings": "设置",
    "status.idle": "等待说话",
    "status.recording": "正在录音...",
    "status.processing": "正在转写...",
    "status.done": "转写完成",
    "main.hint":
      "按住触发键说话，松开后自动转写。快速连按两次同一键可长时间连续录音，再按一次结束。",
    "main.hint_clipboard": "转写完成后文本会写入剪贴板，可直接粘贴使用。",
    "main.copy": "复制到剪贴板",
    "main.result_label": "转写结果",
    "main.key_backend_evtest": "监听方式：本机输入设备（evdev），适用于 Wayland",
    "main.key_backend_xinput": "监听方式：X11 xinput",
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
    "settings.engine_api": "API (OpenAI兼容)",
    "settings.engine_local": "本地 (whisper.cpp)",
    "settings.engine_mimo": "MiMo ASR (小米)",
    "settings.language": "语言",
    "settings.model": "模型",
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
    "settings.delete_model": "删除模型",
    "settings.about": "关于",
    "settings.about_tagline": "按住快捷键说话，松手转写并润色，把口述变成可直接使用的文字。",
    "settings.version": "版本",
    "settings.save": "保存",
    "settings.restart_hint": "保存后会自动重载语音管道（按键监听与转写配置），一般无需重启应用。",
    "settings.transcription_lead": "语音转文字依赖本页配置。本地模式需先下载并选中一个模型；保存后立即生效。",
    "settings.recording_lead":
      "默认使用键盘右侧 Alt。快速连按两次触发键可长时间录音，再按一次结束。若无效请使用「按下以设置」或自定义 keysym。",
    "settings.readiness_api": "云端 API 模式",
    "settings.readiness_api_desc": "请填写有效的 API 地址与密钥后保存。",
    "settings.readiness_mimo": "MiMo ASR 模式",
    "settings.readiness_mimo_desc": "使用小米 MiMo-V2.5-ASR 云端识别，响应快速，支持中英文自动检测。请填写 API Key 后保存。",
    "settings.readiness_local_ok": "本地转写已就绪",
    "settings.readiness_local_need": "请完成本地模型设置",
    "settings.readiness_local_desc": "从下方选择一个模型，点击「下载并启用」；若已下载，点「使用此模型」并保存。",
    "settings.readiness_path_missing": "当前填写的模型名或路径在磁盘上找不到，请重新选择或下载。",
    "settings.key_preset_right_alt": "右Alt",
    "settings.key_custom": "自定义键名…",
    "settings.key_custom_value": "自定义 keysym",
    "settings.key_binding_active": "当前触发键",
    "settings.capture_activation": "按下以设置",
    "settings.capture_activation_short": "按下以设置快捷键",
    "settings.capture_waiting": "请按键…（约 12 秒内）",
    "settings.capture_activation_lead":
      "点击后请按下要作为激活键的按键；成功后会自动保存并重载监听。失败时可再试或手动填写 keysym。",
    "settings.capture_error_title": "按键捕获失败",
    "settings.in_use": "当前",
    "settings.use_model": "使用此模型",
    "settings.current": "已选用",
    "settings.download_and_use": "下载并启用",
    "settings.model_error_title": "模型操作失败",
    "settings.model_download_connecting": "连接中…",
    "settings.advanced_model_path": "高级：自定义路径或模型名",
    "settings.custom_path": "模型名或文件路径",
    "settings.custom_path_placeholder": "例如 base 或 /path/to/ggml-base.bin",
    "settings.custom_path_hint": "可填内置名称（tiny、base…）或本机 .bin 文件的完整路径。",
    "overlay.recording": "录音中...",
    "overlay.transcribing": "转写中...",
    "overlay.polishing": "润色中...",
    "overlay.copy": "复制",
    "overlay.copied": "已复制",
    "overlay.close": "关闭",
    "history.title": "转写历史",
    "history.lead": "仅保存转写文本，不保存录音。记录在本地文件中持久保存，可随时删除。",
    "history.loading": "加载中…",
    "history.empty": "暂无历史记录。完成一次语音转写后会自动出现在此处。",
    "history.select_all": "全选",
    "history.delete_selected": "删除所选",
    "history.clear_all": "清空全部",
    "history.confirm_delete_selected": "确定删除选中的记录？此操作无法撤销。",
    "history.confirm_clear_all": "确定清空全部历史？此操作无法撤销。",
    "history.raw_label": "原始转写",
    "history.copy": "复制",
    "history.copied": "已复制",
    "history.copy_failed": "无法写入剪贴板，请检查系统剪贴板工具或权限后重试。",
    "history.polish": "润色",
    "history.polish_config_missing": "润色功能需要先在设置中配置 API 地址、模型和密钥。",
  },
  en: {
    "title.subtitle": "Voice to Text",
    "nav.home": "Home",
    "nav.history": "History",
    "nav.settings": "Settings",
    "status.idle": "Ready",
    "status.recording": "Recording...",
    "status.processing": "Transcribing...",
    "status.done": "Done",
    "main.hint":
      "Hold the trigger key to speak, release to transcribe. Double-tap quickly for hands-free recording, tap again to stop.",
    "main.hint_clipboard": "When transcription finishes, text is placed on the clipboard—paste it anywhere.",
    "main.copy": "Copy to clipboard",
    "main.result_label": "Transcription",
    "main.key_backend_evtest": "Input: evdev (recommended on Wayland)",
    "main.key_backend_xinput": "Input: X11 xinput",
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
    "settings.engine_api": "API (OpenAI compatible)",
    "settings.engine_local": "Local (whisper.cpp)",
    "settings.engine_mimo": "MiMo ASR (Xiaomi)",
    "settings.language": "Language",
    "settings.model": "Model",
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
    "settings.delete_model": "Delete Model",
    "settings.about": "About",
    "settings.about_tagline": "Hold the shortcut to speak, release to transcribe and polish—turn speech into usable text.",
    "settings.version": "Version",
    "settings.save": "Save",
    "settings.restart_hint": "After saving, the voice pipeline reloads automatically; you usually do not need to restart the app.",
    "settings.transcription_lead": "Transcription uses the settings below. For local mode, download and select a model; changes apply after you save.",
    "settings.recording_lead":
      "Default is Right Alt. Double-tap quickly for hands-free recording, tap again to stop. If the key does not work, use \u201cPress to set\u201d or enter a custom keysym.",
    "settings.readiness_api": "Cloud API mode",
    "settings.readiness_api_desc": "Enter a valid API base URL and key, then save.",
    "settings.readiness_mimo": "MiMo ASR mode",
    "settings.readiness_mimo_desc": "Uses Xiaomi MiMo-V2.5-ASR cloud recognition with fast response and auto language detection. Enter your API Key and save.",
    "settings.readiness_local_ok": "Local transcription ready",
    "settings.readiness_local_need": "Finish local model setup",
    "settings.readiness_local_desc": "Pick a model below and click Download & enable; if already downloaded, click Use this model and save.",
    "settings.readiness_path_missing": "The configured model name or path was not found on disk.",
    "settings.key_preset_right_alt": "Right Alt",
    "settings.key_custom": "Custom keysym…",
    "settings.key_custom_value": "Custom keysym",
    "settings.key_binding_active": "Active trigger key",
    "settings.capture_activation": "Press to set",
    "settings.capture_activation_short": "Set key by pressing",
    "settings.capture_waiting": "Press a key… (within ~12s)",
    "settings.capture_activation_lead":
      "Click, then press the key you want to use. Settings save and the listener reloads automatically. On failure, try again or enter a keysym manually.",
    "settings.capture_error_title": "Key capture failed",
    "settings.in_use": "Active",
    "settings.use_model": "Use this model",
    "settings.current": "Selected",
    "settings.download_and_use": "Download & enable",
    "settings.model_error_title": "Model error",
    "settings.model_download_connecting": "Connecting…",
    "settings.advanced_model_path": "Advanced: custom path or name",
    "settings.custom_path": "Model name or file path",
    "settings.custom_path_placeholder": "e.g. base or /path/to/ggml-base.bin",
    "settings.custom_path_hint": "Use a built-in name (tiny, base, …) or a full path to a .bin file.",
    "overlay.recording": "Recording...",
    "overlay.transcribing": "Transcribing...",
    "overlay.polishing": "Polishing...",
    "overlay.copy": "Copy",
    "overlay.copied": "Copied",
    "overlay.close": "Close",
    "history.title": "Transcription history",
    "history.lead":
      "Only transcribed text is stored—audio is not saved. Records persist in a local file until you delete them.",
    "history.loading": "Loading…",
    "history.empty": "No entries yet. They appear here after you finish a transcription.",
    "history.select_all": "Select all",
    "history.delete_selected": "Delete selected",
    "history.clear_all": "Clear all",
    "history.confirm_delete_selected":
      "Delete the selected entries? This cannot be undone.",
    "history.confirm_clear_all": "Clear all history? This cannot be undone.",
    "history.raw_label": "Raw transcript",
    "history.copy": "Copy",
    "history.copied": "Copied",
    "history.copy_failed":
      "Could not write to the clipboard. Check clipboard tools or permissions and try again.",
    "history.polish": "Polish",
    "history.polish_config_missing": "Polishing requires configuring the API URL, model and key in Settings first.",
  },
};

export function translateStatic(lang: string, key: string): string {
  return translations[lang]?.[key] ?? translations["zh"]?.[key] ?? key;
}

export function useTranslation() {
  const [lang, setLangState] = useState<string>(
    () => localStorage.getItem(LANG_KEY) || "zh"
  );

  useEffect(() => {
    document.documentElement.lang = lang === "en" ? "en" : "zh";
  }, [lang]);

  // Sync language from other same-origin windows (e.g. overlay) via storage events.
  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === LANG_KEY && e.newValue) {
        setLangState(e.newValue);
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  // Sync language within the same window via CustomEvent (like theme.ts).
  useEffect(() => {
    const onLangChanged = () => {
      const v = localStorage.getItem(LANG_KEY) || "zh";
      setLangState(v);
    };
    window.addEventListener("altgo-lang-changed", onLangChanged);
    return () => window.removeEventListener("altgo-lang-changed", onLangChanged);
  }, []);

  const setLang = (code: string) => {
    setLangState(code);
    localStorage.setItem(LANG_KEY, code);
    window.dispatchEvent(new CustomEvent("altgo-lang-changed"));
  };

  const t = (key: string): string => {
    return translateStatic(lang, key);
  };

  return { t, lang, setLang };
}
