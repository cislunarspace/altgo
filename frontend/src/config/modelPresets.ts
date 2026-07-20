/**
 * 模型预设配置
 *
 * 区分两种类型：
 * - polisher: 润色文本模型（LLM chat completion）
 * - transcriber: 语音识别模型（ASR / 多模态）
 */

export type ModelType = "polisher" | "transcriber";

export type ProviderCategory =
  | "official"      // 官方（OpenAI、Anthropic）
  | "cn_official"   // 国产官方（DeepSeek、Kimi、智谱等）
  | "mimo"          // 小米 MiMo
  | "aggregator"    // 聚合服务（OpenRouter、SiliconFlow 等）
  | "custom";       // 自定义

export interface ModelCatalogEntry {
  model: string;
  displayName: string;
  description?: string;
  contextWindow?: number;
  inputModalities?: ("text" | "audio" | "image")[];
  recommended?: boolean;
}

export interface ProviderPreset {
  /** 供应商名称 */
  name: string;
  /** i18n key */
  nameKey?: string;
  /** 官网链接 */
  websiteUrl: string;
  /** 获取 API Key 的链接 */
  apiKeyUrl?: string;
  /** API Base URL */
  apiBaseUrl: string;
  /** 分类 */
  category: ProviderCategory;
  /** 支持的模型类型 */
  modelTypes: ModelType[];
  /** 推荐模型目录 */
  models: ModelCatalogEntry[];
  /** 默认模型 */
  defaultModel: string;
  /** API 协议格式 */
  apiFormat: "openai" | "anthropic" | "mimo_asr";
  /** 图标名称（用于 UI 展示） */
  icon?: string;
  /** 图标颜色 */
  iconColor?: string;
  /** 是否为合作伙伴 */
  isPartner?: boolean;
  /** 置顶合作伙伴 */
  primePartner?: boolean;
  /** 说明文本 i18n key */
  descriptionKey?: string;
}

// ─── 润色模型预设 ──────────────────────────────────────────────────────────

export const polisherPresets: ProviderPreset[] = [
  {
    name: "DeepSeek",
    websiteUrl: "https://platform.deepseek.com",
    apiKeyUrl: "https://platform.deepseek.com/api_keys",
    apiBaseUrl: "https://api.deepseek.com",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "deepseek-chat",
    icon: "deepseek",
    iconColor: "#1E88E5",
    models: [
      {
        model: "deepseek-chat",
        displayName: "DeepSeek Chat",
        description: "通用对话模型，性价比高",
        contextWindow: 128000,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "deepseek-reasoner",
        displayName: "DeepSeek Reasoner",
        description: "推理增强模型，适合复杂任务",
        contextWindow: 128000,
        inputModalities: ["text"],
      },
    ],
  },
  {
    name: "Kimi",
    websiteUrl: "https://platform.moonshot.cn",
    apiKeyUrl: "https://platform.moonshot.cn/console/api-keys",
    apiBaseUrl: "https://api.moonshot.cn/v1",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "moonshot-v1-8k",
    icon: "kimi",
    iconColor: "#000000",
    isPartner: true,
    models: [
      {
        model: "moonshot-v1-8k",
        displayName: "Kimi 8K",
        description: "8K 上下文，速度快",
        contextWindow: 8192,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "moonshot-v1-32k",
        displayName: "Kimi 32K",
        description: "32K 上下文，适合长文",
        contextWindow: 32768,
        inputModalities: ["text"],
      },
      {
        model: "moonshot-v1-128k",
        displayName: "Kimi 128K",
        description: "128K 上下文，超长文处理",
        contextWindow: 131072,
        inputModalities: ["text"],
      },
    ],
  },
  {
    name: "智谱 AI",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://open.bigmodel.cn/usercenter/apikeys",
    apiBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "glm-4-flash",
    icon: "zhipu",
    iconColor: "#2B5CE6",
    models: [
      {
        model: "glm-4-flash",
        displayName: "GLM-4 Flash",
        description: "快速响应，免费额度",
        contextWindow: 128000,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "glm-4-plus",
        displayName: "GLM-4 Plus",
        description: "增强版，更好的效果",
        contextWindow: 128000,
        inputModalities: ["text"],
      },
    ],
  },
  {
    name: "通义千问",
    websiteUrl: "https://dashscope.aliyun.com",
    apiKeyUrl: "https://dashscope.console.aliyun.com/apiKey",
    apiBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "qwen-turbo",
    icon: "qwen",
    iconColor: "#6C3FE8",
    models: [
      {
        model: "qwen-turbo",
        displayName: "Qwen Turbo",
        description: "快速响应，性价比高",
        contextWindow: 128000,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "qwen-plus",
        displayName: "Qwen Plus",
        description: "增强版，更好的效果",
        contextWindow: 128000,
        inputModalities: ["text"],
      },
      {
        model: "qwen-max",
        displayName: "Qwen Max",
        description: "最强模型",
        contextWindow: 32768,
        inputModalities: ["text"],
      },
    ],
  },
  {
    name: "OpenAI",
    websiteUrl: "https://platform.openai.com",
    apiKeyUrl: "https://platform.openai.com/api-keys",
    apiBaseUrl: "https://api.openai.com/v1",
    category: "official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "gpt-4o-mini",
    icon: "openai",
    iconColor: "#10A37F",
    models: [
      {
        model: "gpt-4o-mini",
        displayName: "GPT-4o Mini",
        description: "快速、经济的多模态模型",
        contextWindow: 128000,
        inputModalities: ["text", "image"],
        recommended: true,
      },
      {
        model: "gpt-4o",
        displayName: "GPT-4o",
        description: "旗舰多模态模型",
        contextWindow: 128000,
        inputModalities: ["text", "image"],
      },
    ],
  },
  {
    name: "Anthropic",
    websiteUrl: "https://console.anthropic.com",
    apiKeyUrl: "https://console.anthropic.com/settings/keys",
    apiBaseUrl: "https://api.anthropic.com",
    category: "official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "claude-sonnet-4-20250514",
    icon: "anthropic",
    iconColor: "#D4915D",
    models: [
      {
        model: "claude-sonnet-4-20250514",
        displayName: "Claude Sonnet 4",
        description: "平衡性能与速度",
        contextWindow: 200000,
        inputModalities: ["text", "image"],
        recommended: true,
      },
      {
        model: "claude-haiku-3.5",
        displayName: "Claude Haiku 3.5",
        description: "快速响应，成本低",
        contextWindow: 200000,
        inputModalities: ["text", "image"],
      },
    ],
  },
  {
    name: "SiliconFlow",
    websiteUrl: "https://siliconflow.cn",
    apiKeyUrl: "https://cloud.siliconflow.cn/account/ak",
    apiBaseUrl: "https://api.siliconflow.cn/v1",
    category: "aggregator",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "Qwen/Qwen2.5-7B-Instruct",
    icon: "siliconflow",
    iconColor: "#4F46E5",
    models: [
      {
        model: "Qwen/Qwen2.5-7B-Instruct",
        displayName: "Qwen 2.5 7B",
        description: "免费模型，适合轻度润色",
        contextWindow: 32768,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "deepseek-ai/DeepSeek-V3",
        displayName: "DeepSeek V3",
        description: "高质量通用模型",
        contextWindow: 65536,
        inputModalities: ["text"],
      },
    ],
  },
];

// ─── 语音识别模型预设 ──────────────────────────────────────────────────────

export const transcriberPresets: ProviderPreset[] = [
  {
    name: "MiMo ASR",
    nameKey: "preset.mimo_asr",
    websiteUrl: "https://mimo.mi.com",
    apiKeyUrl: "https://platform.xiaomimimo.com/console/apikeys",
    apiBaseUrl: "https://api.xiaomimimo.com/v1",
    category: "mimo",
    modelTypes: ["transcriber"],
    apiFormat: "mimo_asr",
    defaultModel: "mimo-v2.5-asr",
    icon: "mimo",
    iconColor: "#FF6900",
    primePartner: true,
    descriptionKey: "preset.mimo_asr_desc",
    models: [
      {
        model: "mimo-v2.5-asr",
        displayName: "MiMo V2.5 ASR",
        description: "小米多模态语音识别，支持中英文自动检测、方言、噪声环境",
        inputModalities: ["audio"],
        recommended: true,
      },
    ],
  },
  {
    name: "OpenAI Whisper",
    websiteUrl: "https://platform.openai.com",
    apiKeyUrl: "https://platform.openai.com/api-keys",
    apiBaseUrl: "https://api.openai.com/v1",
    category: "official",
    modelTypes: ["transcriber"],
    apiFormat: "openai",
    defaultModel: "whisper-1",
    icon: "openai",
    iconColor: "#10A37F",
    models: [
      {
        model: "whisper-1",
        displayName: "Whisper",
        description: "OpenAI 语音识别模型，支持多语言",
        inputModalities: ["audio"],
        recommended: true,
      },
    ],
  },
  {
    name: "本地 whisper.cpp",
    nameKey: "preset.local_whisper",
    websiteUrl: "https://github.com/ggerganov/whisper.cpp",
    apiBaseUrl: "",
    category: "custom",
    modelTypes: ["transcriber"],
    apiFormat: "openai",
    defaultModel: "base",
    icon: "local",
    iconColor: "#6B7280",
    descriptionKey: "preset.local_whisper_desc",
    models: [
      {
        model: "tiny",
        displayName: "Tiny",
        description: "最小模型，速度最快（75MB）",
        inputModalities: ["audio"],
      },
      {
        model: "base",
        displayName: "Base",
        description: "基础模型，推荐日常使用（142MB）",
        inputModalities: ["audio"],
        recommended: true,
      },
      {
        model: "small",
        displayName: "Small",
        description: "小型模型，更好的准确率（466MB）",
        inputModalities: ["audio"],
      },
      {
        model: "medium",
        displayName: "Medium",
        description: "中型模型，推荐中文使用（1.5GB）",
        inputModalities: ["audio"],
        recommended: true,
      },
      {
        model: "large-v3",
        displayName: "Large V3",
        description: "大型模型，最佳准确率（2.8GB）",
        inputModalities: ["audio"],
      },
    ],
  },
];

// ─── 分类显示顺序和标签 ──────────────────────────────────────────────────────

export const categoryOrder: ProviderCategory[] = [
  "mimo",
  "official",
  "cn_official",
  "aggregator",
  "custom",
];

export const categoryLabels: Record<ProviderCategory, string> = {
  official: "官方",
  cn_official: "国产",
  mimo: "MiMo",
  aggregator: "聚合服务",
  custom: "本地/自定义",
};

export const categoryLabelsEn: Record<ProviderCategory, string> = {
  official: "Official",
  cn_official: "Chinese",
  mimo: "MiMo",
  aggregator: "Aggregator",
  custom: "Local/Custom",
};
