/**
 * 在线供应商目录：从 Oh My Pi 的模型目录（catalog.stencil.so，与 `omp models` 同源）
 * 拉取全部供应商与模型，转成与本地预置同构的 ProviderPreset，注入预设选择器。
 *
 * Online provider catalog: fetched from Oh My Pi's model catalog (catalog.stencil.so, the
 * same source `omp models` uses) and converted into ProviderPreset entries compatible with
 * the local presets, then fed into the preset picker.
 */
import type { ModelCatalogEntry, ProviderPreset } from "./modelPresets";

const CATALOG_URL = "https://catalog.stencil.so/models.json";

/**
 * 目录条目不带协议字段（omp 在自身代码内映射）；altgo 按 provider id 定协议，
 * 未命中的默认 OpenAI 兼容——设置页的协议下拉始终可以手动纠正。
 *
 * The catalog carries no protocol field (omp maps it in its own code); we map by provider
 * id and default to OpenAI-compatible — the protocol dropdown can always correct it.
 */
const ANTHROPIC_PROVIDERS: Record<string, true> = {
  anthropic: true,
  "zai-coding-plan": true,
  "zhipuai-coding-plan": true,
};

/**
 * 少数大厂在目录里不写 base URL（官方 SDK 默认地址），这里补上，避免被跳过。
 *
 * A few majors ship no base URL in the catalog (SDK defaults); fill them in so they are
 * not skipped.
 */
const OFFICIAL_BASE_URLS: Record<string, string> = {
  anthropic: "https://api.anthropic.com",
  openai: "https://api.openai.com/v1",
  google: "https://generativelanguage.googleapis.com/v1beta/openai",
};

interface CatalogModel {
  id: string;
  name?: string;
  limit?: { context?: number };
}

interface CatalogProvider {
  api?: string;
  models?: Record<string, CatalogModel>;
}

/**
 * 把目录 JSON 转成 ProviderPreset 列表：
 * - 无端点且不在官方补全表的条目跳过（如 Vertex，需要项目级配置）；
 * - 与本地预置同端点的条目跳过（避免重复展示）；
 * - 模型按显示名排序，上下文窗口取自 limit.context。
 *
 * Converts catalog JSON into ProviderPreset entries: entries without an endpoint (and not
 * in the official-URL table) are skipped, entries duplicating a local preset's endpoint are
 * dropped, models sort by display name with the context window from limit.context.
 */
export function parseCatalog(json: unknown, existing: ProviderPreset[]): ProviderPreset[] {
  if (typeof json !== "object" || json === null) return [];
  const knownUrls = new Set(existing.map((preset) => preset.apiBaseUrl));
  const presets: ProviderPreset[] = [];

  for (const [key, value] of Object.entries(json as Record<string, CatalogProvider>)) {
    const api = (value?.api ?? OFFICIAL_BASE_URLS[key])?.replace(/\/+$/, "");
    if (!api || knownUrls.has(api)) continue;

    const models: ModelCatalogEntry[] = Object.values(value.models ?? {})
      .map((model) => ({
        model: model.id,
        displayName: model.name || model.id,
        contextWindow: model.limit?.context,
      }))
      .sort((a, b) => a.displayName.localeCompare(b.displayName));

    presets.push({
      name: key,
      websiteUrl: "",
      apiBaseUrl: api,
      category: "custom",
      modelTypes: ["polisher"],
      apiFormat: ANTHROPIC_PROVIDERS[key] ? "anthropic" : "openai",
      models,
      defaultModel: models[0]?.model ?? "",
    });
  }
  return presets.sort((a, b) => a.name.localeCompare(b.name));
}

let cached: Promise<ProviderPreset[]> | null = null;

/**
 * 拉取并解析目录；会话内只拉一次，失败后允许重试。
 * Fetches and parses the catalog once per session; failures reset the cache to allow retries.
 */
export function loadCatalog(existing: ProviderPreset[]): Promise<ProviderPreset[]> {
  if (!cached) {
    cached = fetch(CATALOG_URL)
      .then((res) => (res.ok ? res.json() : Promise.reject(new Error(`HTTP ${res.status}`))))
      .then((json) => parseCatalog(json, existing))
      .catch((error: unknown) => {
        cached = null;
        throw error;
      });
  }
  return cached;
}
