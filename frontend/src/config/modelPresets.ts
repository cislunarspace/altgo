/**
 * 模型预设配置。
 *
 * 当前只保留文本润色供应商预设；本地转写使用 SenseVoice，无云端供应商。
 */

export type ModelType = "polisher";

export type ProviderCategory =
  | "official"      // 官方（OpenAI、Anthropic）
  | "cn_official"   // 国产官方（DeepSeek、Kimi、智谱等）
  | "aggregator"    // 聚合服务（OpenRouter、SiliconFlow 等）
  | "third_party"   // 第三方中转
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
  apiFormat: "openai" | "anthropic";
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
    defaultModel: "deepseek-v4-flash",
    icon: "deepseek",
    iconColor: "#1E88E5",
    models: [
      {
        model: "deepseek-v4-flash",
        displayName: "DeepSeek V4 Flash",
        description: "轻量快速，适合语音润色",
        contextWindow: 1000000,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "deepseek-v4-pro",
        displayName: "DeepSeek V4 Pro",
        description: "旗舰模型，适合重度改写",
        contextWindow: 1000000,
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
    defaultModel: "kimi-k2.6",
    icon: "kimi",
    iconColor: "#000000",
    isPartner: true,
    models: [
      {
        model: "kimi-k2.6",
        displayName: "Kimi K2.6",
        description: "当前通用主力，速度快，适合润色",
        contextWindow: 131072,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "kimi-k3",
        displayName: "Kimi K3",
        description: "旗舰模型，效果更强、稍慢",
        contextWindow: 1048576,
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
    defaultModel: "glm-5.3-flash",
    icon: "zhipu",
    iconColor: "#2B5CE6",
    models: [
      {
        model: "glm-5.3-flash",
        displayName: "GLM-5.3 Flash",
        description: "免费，速度快，适合润色",
        contextWindow: 1000000,
        inputModalities: ["text"],
        recommended: true,
      },
      {
        model: "glm-5.2",
        displayName: "GLM-5.2",
        description: "旗舰，效果更强",
        contextWindow: 200000,
        inputModalities: ["text"],
      },
      {
        model: "glm-5.1",
        displayName: "GLM-5.1",
        description: "上一代旗舰，价格更低",
        contextWindow: 198000,
        inputModalities: ["text"],
      },
    ],
  },
  {
    name: "Kimi (Anthropic)",
    websiteUrl: "https://platform.kimi.com",
    apiBaseUrl: "https://api.moonshot.cn/anthropic",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "kimi-k2.7-code",
    iconColor: "#6366F1",
    models: [
      { model: "kimi-k2.7-code", displayName: "kimi-k2.7-code", recommended: true },
    ],
  },
  {
    name: "Kimi For Coding",
    websiteUrl: "https://www.kimi.com/code",
    apiBaseUrl: "https://api.kimi.com/coding/",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "kimi-for-coding",
    iconColor: "#6366F1",
    models: [
      { model: "kimi-for-coding", displayName: "kimi-for-coding", recommended: true },
    ],
  },
  {
    name: "DeepSeek (Anthropic)",
    websiteUrl: "https://platform.deepseek.com",
    apiBaseUrl: "https://api.deepseek.com/anthropic",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "deepseek-v4-pro",
    iconColor: "#1E88E5",
    models: [
      { model: "deepseek-v4-pro", displayName: "deepseek-v4-pro", recommended: true },
    ],
  },
  {
    name: "Zhipu GLM",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://www.bigmodel.cn/claude-code",
    apiBaseUrl: "https://open.bigmodel.cn/api/anthropic",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "glm-5.1",
    iconColor: "#0F62FE",
    models: [
      { model: "glm-5.1", displayName: "glm-5.1", recommended: true },
    ],
  },
  {
    name: "Zhipu GLM en",
    websiteUrl: "https://z.ai",
    apiKeyUrl: "https://z.ai/subscribe",
    apiBaseUrl: "https://api.z.ai/api/anthropic",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "glm-5.1",
    iconColor: "#0F62FE",
    models: [
      { model: "glm-5.1", displayName: "glm-5.1", recommended: true },
    ],
  },
  {
    name: "Xiaomi MiMo",
    websiteUrl: "https://platform.xiaomimimo.com",
    apiKeyUrl: "https://platform.xiaomimimo.com",
    apiBaseUrl: "https://api.xiaomimimo.com/anthropic",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "mimo-v2.5-pro",
    iconColor: "#000000",
    models: [
      { model: "mimo-v2.5-pro", displayName: "mimo-v2.5-pro", recommended: true },
    ],
  },
  {
    name: "Xiaomi MiMo Token Plan (China)",
    websiteUrl: "https://platform.xiaomimimo.com",
    apiKeyUrl: "https://platform.xiaomimimo.com",
    apiBaseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "anthropic",
    defaultModel: "mimo-v2.5-pro",
    iconColor: "#000000",
    models: [
      { model: "mimo-v2.5-pro", displayName: "mimo-v2.5-pro", recommended: true },
    ],
  },
  {
    name: "Kimi For Coding (OpenAI)",
    websiteUrl: "https://www.kimi.com/code",
    apiKeyUrl: "https://www.kimi.com/code",
    apiBaseUrl: "https://api.kimi.com/coding/v1",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "kimi-for-coding",
    iconColor: "#6366F1",
    models: [
      { model: "kimi-for-coding", displayName: "Kimi For Coding", contextWindow: 262144 },
      { model: "kimi-for-coding-highspeed", displayName: "Kimi For Coding HighSpeed", contextWindow: 262144 },
      { model: "k3", displayName: "Kimi K3", contextWindow: 1048576 },
      { model: "k3-256k", displayName: "Kimi K3 256K", contextWindow: 262144 },
    ],
  },
  {
    name: "Zhipu GLM (OpenAI)",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://www.bigmodel.cn/claude-code",
    apiBaseUrl: "https://open.bigmodel.cn/api/coding/paas/v4",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "glm-5.3-flash",
    iconColor: "#0F62FE",
    models: [
      { model: "glm-5.3-flash", displayName: "GLM-5.3 Flash", contextWindow: 1000000 },
      { model: "glm-5.2", displayName: "GLM-5.2", contextWindow: 200000 },
    ],
  },
  {
    name: "Zhipu GLM en (OpenAI)",
    websiteUrl: "https://z.ai",
    apiKeyUrl: "https://z.ai/subscribe",
    apiBaseUrl: "https://api.z.ai/api/coding/paas/v4",
    category: "cn_official",
    modelTypes: ["polisher"],
    apiFormat: "openai",
    defaultModel: "glm-5.3-flash",
    iconColor: "#0F62FE",
    models: [
      { model: "glm-5.3-flash", displayName: "GLM-5.3 Flash", contextWindow: 1000000 },
      { model: "glm-5.2", displayName: "GLM-5.2", contextWindow: 200000 },
    ],
  },
];

export const categoryOrder: ProviderCategory[] = [
  "official",
  "cn_official",
  "aggregator",
  "third_party",
  "custom",
];

export const categoryLabels: Record<ProviderCategory, string> = {
  official: "官方",
  cn_official: "国产",
  aggregator: "聚合服务",
  third_party: "第三方中转",
  custom: "本地/自定义",
};

export const categoryLabelsEn: Record<ProviderCategory, string> = {
  official: "Official",
  cn_official: "Chinese",
  aggregator: "Aggregator",
  third_party: "Third-party",
  custom: "Local/Custom",
};
