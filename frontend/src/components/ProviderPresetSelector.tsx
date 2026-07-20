import { useState, useMemo } from "react";
import { ChevronDown, ChevronRight, ExternalLink, Search, Star, Heart } from "lucide-react";
import type {
  ProviderPreset,
  ProviderCategory,
  ModelType,
  ModelCatalogEntry,
} from "../config/modelPresets";
import { categoryOrder, categoryLabels, categoryLabelsEn } from "../config/modelPresets";

interface Props {
  presets: ProviderPreset[];
  modelType: ModelType;
  currentApiBaseUrl: string;
  currentModel: string;
  lang: string;
  t: (key: string) => string;
  onSelect: (preset: ProviderPreset, model?: ModelCatalogEntry) => void;
}

export function ProviderPresetSelector({
  presets,
  modelType: _modelType,
  currentApiBaseUrl,
  currentModel,
  lang,
  t,
  onSelect,
}: Props) {
  const [search, setSearch] = useState("");
  const [expandedCategory, setExpandedCategory] = useState<ProviderCategory | null>(null);
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);

  // 按分类分组
  const grouped = useMemo(() => {
    const filtered = presets.filter(
      (p) =>
        p.name.toLowerCase().includes(search.toLowerCase()) ||
        p.models.some((m) =>
          m.displayName.toLowerCase().includes(search.toLowerCase())
        )
    );

    const groups = new Map<ProviderCategory, ProviderPreset[]>();
    for (const cat of categoryOrder) {
      const items = filtered.filter((p) => p.category === cat);
      if (items.length > 0) {
        groups.set(cat, items);
      }
    }
    return groups;
  }, [presets, search]);

  // 检查是否为当前选中的预设
  const isActive = (preset: ProviderPreset) =>
    preset.apiBaseUrl === currentApiBaseUrl ||
    currentApiBaseUrl.includes(preset.apiBaseUrl.replace("https://", "").split("/")[0]);

  const labels = lang === "zh" ? categoryLabels : categoryLabelsEn;

  return (
    <div className="provider-preset-selector">
      <div className="provider-preset-search">
        <Search size={14} />
        <input
          type="text"
          placeholder={t("settings.search_providers")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="provider-preset-list">
        {Array.from(grouped.entries()).map(([category, providers]) => (
          <div key={category} className="provider-preset-category">
            <button
              type="button"
              className="provider-preset-category-header"
              onClick={() =>
                setExpandedCategory(expandedCategory === category ? null : category)
              }
            >
              {expandedCategory === category ? (
                <ChevronDown size={12} />
              ) : (
                <ChevronRight size={12} />
              )}
              <span className="provider-preset-category-label">
                {labels[category]}
              </span>
              <span className="provider-preset-category-count">
                {providers.length}
              </span>
            </button>

            {(expandedCategory === category || search.length > 0) && (
              <div className="provider-preset-items">
                {providers.map((preset) => (
                  <div
                    key={preset.name}
                    className={`provider-preset-item ${isActive(preset) ? "is-active" : ""}`}
                  >
                    <button
                      type="button"
                      className="provider-preset-header"
                      onClick={() =>
                        setExpandedProvider(
                          expandedProvider === preset.name ? null : preset.name
                        )
                      }
                    >
                      <div className="provider-preset-info">
                        <span
                          className="provider-preset-icon"
                          style={{
                            backgroundColor: preset.iconColor || "#6B7280",
                          }}
                        >
                          {preset.name.charAt(0)}
                        </span>
                        <span className="provider-preset-name">
                          {preset.name}
                        </span>
                        {preset.primePartner && (
                          <Heart size={12} className="provider-preset-badge provider-preset-badge--prime" />
                        )}
                        {preset.isPartner && !preset.primePartner && (
                          <Star size={12} className="provider-preset-badge provider-preset-badge--partner" />
                        )}
                      </div>
                      {preset.websiteUrl && (
                        <a
                          href={preset.websiteUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="provider-preset-link"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <ExternalLink size={11} />
                        </a>
                      )}
                    </button>

                    {expandedProvider === preset.name && (
                      <div className="provider-preset-detail">
                        {preset.descriptionKey && (
                          <p className="provider-preset-desc">
                            {t(preset.descriptionKey)}
                          </p>
                        )}

                        {preset.apiKeyUrl && (
                          <a
                            href={preset.apiKeyUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="provider-preset-apikey-link"
                          >
                            {t("settings.get_api_key")} →
                          </a>
                        )}

                        <div className="provider-preset-models">
                          <span className="provider-preset-models-label">
                            {t("settings.recommended_models")}
                          </span>
                          {preset.models.map((model) => (
                            <button
                              key={model.model}
                              type="button"
                              className={`provider-preset-model ${model.recommended ? "is-recommended" : ""} ${currentModel === model.model ? "is-active" : ""}`}
                              onClick={() => onSelect(preset, model)}
                            >
                              <span className="provider-preset-model-name">
                                {model.displayName}
                              </span>
                              {model.description && (
                                <span className="provider-preset-model-desc">
                                  {model.description}
                                </span>
                              )}
                              {model.recommended && (
                                <span className="provider-preset-model-badge">
                                  推荐
                                </span>
                              )}
                            </button>
                          ))}
                        </div>

                        <button
                          type="button"
                          className="settings-btn settings-btn-sm settings-btn-primary provider-preset-use"
                          onClick={() => onSelect(preset)}
                        >
                          使用此供应商
                        </button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
