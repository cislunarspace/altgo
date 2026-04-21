import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message as showMessageDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../i18n";
import { useModelDownloadProgress } from "../hooks/useTauri";
import {
  Save,
  Globe,
  Mic,
  Sparkles,
  Check,
  Download,
  Trash2,
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Palette,
  Keyboard,
} from "lucide-react";
import { useTheme, type ThemePref } from "../ThemeContext";
import "../styles/components.css";

/** Matches backend ConfigResponse (camelCase). */
interface AppConfig {
  keyName: string;
  /** Linux evdev 键码；由「按下以设置」写入 */
  linuxEvdevCode: number | null;
  /** Windows 虚拟键码 */
  windowsVk: number | null;
  language: string;
  engine: string;
  model: string;
  apiBaseUrl: string;
  polishLevel: string;
  polishModel: string;
  polishApiBaseUrl: string;
  guiLanguage: string;
  transcriberApiKey: string;
  polisherApiKey: string;
}

interface ModelEntry {
  name: string;
  filename: string;
  sizeBytes: number;
  description: string;
  downloaded: boolean;
}

const KEY_PRESETS: { value: string; labelKey: string }[] = [
  { value: "Alt_R", labelKey: "settings.key_preset_right_alt" },
];

/** 与下拉「右Alt」一致的老 keysym 名，仍视为预设而非自定义输入 */
function isPresetKeyName(keyName: string): boolean {
  if (KEY_PRESETS.some((p) => p.value === keyName)) return true;
  return keyName === "ISO_Level3_Shift" || keyName === "AltGr";
}

/** 下拉框受控 value：老配置里的右 Alt keysym 显示为「右Alt」 */
function presetSelectValue(keyName: string): string {
  if (KEY_PRESETS.some((p) => p.value === keyName)) return keyName;
  if (keyName === "ISO_Level3_Shift" || keyName === "AltGr") return "Alt_R";
  return "__custom__";
}

function formatSize(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

function saveRequestBody(c: AppConfig) {
  return {
    keyName: c.keyName,
    linuxEvdevCode: c.linuxEvdevCode,
    windowsVk: c.windowsVk,
    language: c.language,
    engine: c.engine,
    model: c.model,
    apiKey: c.transcriberApiKey,
    apiBaseUrl: c.apiBaseUrl,
    polishLevel: c.polishLevel,
    polishModel: c.polishModel,
    polishApiKey: c.polisherApiKey,
    polishApiBaseUrl: c.polishApiBaseUrl,
    guiLanguage: c.guiLanguage,
  };
}

function normalizeConfig(c: AppConfig): AppConfig {
  return {
    ...c,
    linuxEvdevCode: c.linuxEvdevCode ?? null,
    windowsVk: c.windowsVk ?? null,
  };
}

export default function Settings() {
  const { t, setLang } = useTranslation();
  const { themePref, setTheme } = useTheme();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [resolvedPath, setResolvedPath] = useState<string | null | undefined>(undefined);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<"saved" | string>("");
  const [polishOpen, setPolishOpen] = useState(false);
  const [advancedPath, setAdvancedPath] = useState(false);
  const [keyCapturing, setKeyCapturing] = useState(false);
  const progress = useModelDownloadProgress();

  const refreshModels = useCallback(() => {
    invoke<ModelEntry[]>("list_models").then(setModels).catch(() => {});
  }, []);

  const reportModelFailure = useCallback(
    async (err: unknown) => {
      await showMessageDialog(String(err), {
        title: t("settings.model_error_title"),
        kind: "error",
      });
    },
    [t],
  );

  const refreshResolved = useCallback((model: string, engine: string) => {
    if (engine !== "local" || !model.trim()) {
      setResolvedPath(null);
      return;
    }
    invoke<string | null>("resolve_model", { model })
      .then(setResolvedPath)
      .catch(() => setResolvedPath(null));
  }, []);

  useEffect(() => {
    invoke<AppConfig>("get_config")
      .then((c) => {
        setConfig(normalizeConfig(c));
        refreshResolved(c.model, c.engine);
      })
      .catch(() => {});
    refreshModels();
  }, [refreshModels, refreshResolved]);

  useEffect(() => {
    if (!config) return;
    refreshResolved(config.model, config.engine);
  }, [config?.model, config?.engine, refreshResolved]);

  const update = <K extends keyof AppConfig>(key: K, value: AppConfig[K]) => {
    setConfig((prev) => (prev ? { ...prev, [key]: value } : prev));
  };

  const save = async () => {
    if (!config) return;
    setSaving(true);
    setMessage("");
    try {
      await invoke("save_config", {
        req: saveRequestBody(config),
      });
      setLang(config.guiLanguage);
      setMessage("saved");
      refreshResolved(config.model, config.engine);
      refreshModels();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  };

  const downloadAndUse = async (name: string) => {
    setDownloading(name);
    type FinishPayload = {
      name: string;
      success: boolean;
      path?: string;
      error?: string;
    };
    let resolveFinished!: (v: {
      success: boolean;
      path?: string;
      error?: string;
    }) => void;
    const finished = new Promise<{
      success: boolean;
      path?: string;
      error?: string;
    }>((resolve) => {
      resolveFinished = resolve;
    });

    const unlisten = await listen<FinishPayload>("model-download-finished", (event) => {
      const p = event.payload;
      if (p.name !== name) return;
      resolveFinished({
        success: p.success,
        path: p.path,
        error: p.error,
      });
    });

    try {
      await invoke("download_model", { name });
      const result = await finished;

      if (!result.success) {
        await reportModelFailure(result.error ?? "download failed");
        return;
      }

      const updated = await invoke<ModelEntry[]>("list_models");
      setModels(updated);
      if (!config) return;
      const next = { ...config, engine: "local", model: name };
      setConfig(next);
      await invoke("save_config", {
        req: saveRequestBody({ ...next, model: name }),
      });
      setLang(next.guiLanguage);
      setMessage("saved");
      refreshResolved(name, "local");
    } catch (e) {
      await reportModelFailure(e);
    } finally {
      unlisten();
      setDownloading(null);
    }
  };

  const handleDelete = async (name: string) => {
    try {
      await invoke("delete_model", { name });
      refreshModels();
      if (config?.model === name) {
        update("model", "");
      }
    } catch (e) {
      await reportModelFailure(e);
    }
  };

  const applyLocalModel = async (name: string) => {
    if (!config) return;
    const next = { ...config, engine: "local", model: name };
    setConfig(next);
    setSaving(true);
    setMessage("");
    try {
      await invoke("save_config", {
        req: saveRequestBody({ ...next, model: name }),
      });
      setLang(next.guiLanguage);
      setMessage("saved");
      refreshResolved(name, "local");
      refreshModels();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  };

  const captureActivationKey = async () => {
    if (!config) return;
    setKeyCapturing(true);
    setMessage("");
    try {
      const r = await invoke<{
        keyName: string;
        linuxEvdevCode?: number | null;
        windowsVk?: number | null;
      }>("capture_activation_key");
      const next: AppConfig = {
        ...config,
        keyName: r.keyName,
        linuxEvdevCode: r.linuxEvdevCode ?? null,
        windowsVk: r.windowsVk ?? null,
      };
      setConfig(next);
      await invoke("save_config", { req: saveRequestBody(next) });
      setLang(next.guiLanguage);
      setMessage("saved");
      refreshResolved(next.model, next.engine);
      refreshModels();
    } catch (e) {
      setMessage(String(e));
      await invoke("start_pipeline").catch(() => {});
    } finally {
      setKeyCapturing(false);
    }
  };

  const downloadingProgress = (() => {
    if (!downloading) return 0;
    const meta = models.find((m) => m.name === downloading);
    const total =
      progress.name === downloading && progress.total > 0
        ? progress.total
        : meta?.sizeBytes ?? Math.max(progress.total, 1);
    const downloaded = progress.name === downloading ? progress.downloaded : 0;
    return Math.round((downloaded / Math.max(total, 1)) * 100);
  })();

  const showDownloadConnecting =
    !!downloading &&
    (progress.name !== downloading ||
      (progress.name === downloading && progress.downloaded === 0));

  if (!config) {
    return <div className="loading-container">{t("settings.loading")}</div>;
  }

  const localReady =
    config.engine === "local" && resolvedPath != null && resolvedPath !== "";
  const localBlocked =
    config.engine === "local" &&
    config.model.trim() !== "" &&
    resolvedPath === null;

  return (
    <div className="settings-page settings-page--v2">
      {/* Readiness — implicit state made explicit */}
      <div
        className={`settings-readiness ${
          config.engine === "api"
            ? "settings-readiness--neutral"
            : localReady
              ? "settings-readiness--ok"
              : "settings-readiness--warn"
        }`}
      >
        <div className="settings-readiness-icon">
          {config.engine === "api" ? (
            <Sparkles size={18} />
          ) : localReady ? (
            <CheckCircle2 size={18} />
          ) : (
            <AlertCircle size={18} />
          )}
        </div>
        <div className="settings-readiness-text">
          <strong className="settings-readiness-title">
            {config.engine === "api"
              ? t("settings.readiness_api")
              : localReady
                ? t("settings.readiness_local_ok")
                : t("settings.readiness_local_need")}
          </strong>
          <p className="settings-readiness-desc">
            {config.engine === "api"
              ? t("settings.readiness_api_desc")
              : localBlocked
                ? t("settings.readiness_path_missing")
                : t("settings.readiness_local_desc")}
          </p>
          {config.engine === "local" && resolvedPath && (
            <code className="settings-readiness-path">{resolvedPath}</code>
          )}
        </div>
      </div>

      <div className="settings-form">
        {/* Transcription — primary */}
        <section className="settings-section settings-section--primary">
          <h3 className="settings-section-title">
            <Sparkles size={14} />
            {t("settings.transcription")}
          </h3>
          <p className="settings-section-lead">{t("settings.transcription_lead")}</p>

          <div className="settings-engine-switch">
            <button
              type="button"
              className={`settings-engine-btn ${config.engine === "local" ? "is-active" : ""}`}
              onClick={() => update("engine", "local")}
            >
              {t("settings.engine_local")}
            </button>
            <button
              type="button"
              className={`settings-engine-btn ${config.engine === "api" ? "is-active" : ""}`}
              onClick={() => update("engine", "api")}
            >
              {t("settings.engine_api")}
            </button>
          </div>

          {config.engine === "local" ? (
            <>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.language")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.language}
                    onChange={(e) => update("language", e.target.value)}
                    placeholder="zh"
                  />
                </div>
              </div>

              <div className="settings-model-grid">
                {models.map((m) => {
                  const isActive = config.model === m.name;
                  return (
                    <div
                      key={m.name}
                      className={`settings-model-card ${isActive ? "is-active" : ""}`}
                    >
                      <div className="settings-model-card-head">
                        <span className="settings-model-card-name">{m.name}</span>
                        {isActive && (
                          <span className="settings-model-card-badge">{t("settings.in_use")}</span>
                        )}
                      </div>
                      <p className="settings-model-card-desc">{m.description}</p>
                      <p className="settings-model-card-meta">
                        {formatSize(m.sizeBytes)} · {m.filename}
                      </p>
                      <div className="settings-model-card-actions">
                        {m.downloaded ? (
                          <>
                            <button
                              type="button"
                              className="settings-btn settings-btn-sm settings-btn-secondary"
                              onClick={() => applyLocalModel(m.name)}
                              disabled={isActive || saving}
                            >
                              {isActive ? t("settings.current") : t("settings.use_model")}
                            </button>
                            <button
                              type="button"
                              className="settings-btn settings-btn-sm settings-btn-danger"
                              onClick={() => handleDelete(m.name)}
                            >
                              <Trash2 size={11} />
                              {t("settings.delete_model")}
                            </button>
                          </>
                        ) : downloading === m.name ? (
                          <div className="model-progress" style={{ width: "100%" }}>
                            <div className="progress-bar">
                              <div
                                className="progress-fill"
                                style={{ width: `${downloadingProgress}%` }}
                              />
                            </div>
                            <span className="progress-text">
                              {showDownloadConnecting
                                ? t("settings.model_download_connecting")
                                : `${downloadingProgress}%`}
                            </span>
                          </div>
                        ) : (
                          <button
                            type="button"
                            className="settings-btn settings-btn-sm settings-btn-primary"
                            onClick={() => downloadAndUse(m.name)}
                            disabled={downloading !== null}
                          >
                            <Download size={11} />
                            {t("settings.download_and_use")}
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
              <button
                type="button"
                className="settings-advanced-toggle"
                onClick={() => setAdvancedPath(!advancedPath)}
              >
                {advancedPath ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                {t("settings.advanced_model_path")}
              </button>
              {advancedPath && (
                <div className="settings-field settings-field--nested">
                  <span className="settings-field-label-text">{t("settings.custom_path")}</span>
                  <div className="settings-field-control">
                    <input
                      type="text"
                      className="settings-input"
                      value={config.model}
                      onChange={(e) => update("model", e.target.value)}
                      placeholder={t("settings.custom_path_placeholder")}
                    />
                  </div>
                  <p className="settings-hint">{t("settings.custom_path_hint")}</p>
                </div>
              )}
            </>
          ) : (
            <>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.api_url")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.apiBaseUrl}
                    onChange={(e) => update("apiBaseUrl", e.target.value)}
                    placeholder="https://api.openai.com"
                  />
                </div>
              </div>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.api_key")}</span>
                <div className="settings-field-control">
                  <input
                    type="password"
                    className="settings-input"
                    value={config.transcriberApiKey}
                    onChange={(e) => update("transcriberApiKey", e.target.value)}
                    placeholder="sk-..."
                  />
                </div>
              </div>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.model")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.model}
                    onChange={(e) => update("model", e.target.value)}
                    placeholder="whisper-1"
                  />
                </div>
              </div>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.language")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.language}
                    onChange={(e) => update("language", e.target.value)}
                    placeholder="zh"
                  />
                </div>
              </div>
            </>
          )}
        </section>

        {/* Recording */}
        <section className="settings-section">
          <h3 className="settings-section-title">
            <Mic size={14} />
            {t("settings.recording")}
          </h3>
          <p className="settings-section-lead">{t("settings.recording_lead")}</p>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.key_name")}</span>
            <div className="settings-field-control settings-field-control--trigger-key">
              <select
                className="settings-select"
                value={presetSelectValue(config.keyName)}
                onChange={(e) => {
                  if (e.target.value === "__custom__") return;
                  setConfig((prev) =>
                    prev
                      ? {
                          ...prev,
                          keyName: e.target.value,
                          linuxEvdevCode: null,
                          windowsVk: null,
                        }
                      : prev,
                  );
                }}
              >
                {KEY_PRESETS.map((p) => (
                  <option key={p.value} value={p.value}>
                    {t(p.labelKey)}
                  </option>
                ))}
                <option value="__custom__">{t("settings.key_custom")}</option>
              </select>
              {!isPresetKeyName(config.keyName) && (
                <div className="settings-key-binding-readout">
                  <span className="settings-muted">{t("settings.key_binding_active")}</span>
                  <code className="settings-key-binding-code">{config.keyName}</code>
                </div>
              )}
            </div>
          </div>
          {!isPresetKeyName(config.keyName) && config.linuxEvdevCode == null && config.windowsVk == null && (
            <div className="settings-field">
              <span className="settings-field-label-text">{t("settings.key_custom_value")}</span>
              <div className="settings-field-control">
                <input
                  type="text"
                  className="settings-input"
                  value={config.keyName}
                  onChange={(e) =>
                    setConfig((prev) =>
                      prev
                        ? {
                            ...prev,
                            keyName: e.target.value,
                            linuxEvdevCode: null,
                            windowsVk: null,
                          }
                        : prev,
                    )
                  }
                />
              </div>
            </div>
          )}
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.capture_activation")}</span>
            <div className="settings-field-control">
              <button
                type="button"
                className="settings-btn settings-btn-secondary"
                onClick={() => void captureActivationKey()}
                disabled={saving || keyCapturing}
              >
                <Keyboard size={14} />
                {keyCapturing ? t("settings.capture_waiting") : t("settings.capture_activation_short")}
              </button>
            </div>
          </div>
          <p className="settings-hint">{t("settings.capture_activation_lead")}</p>
        </section>

        {/* Polishing — collapsible */}
        <section className="settings-section">
          <button
            type="button"
            className="settings-section-toggle"
            onClick={() => setPolishOpen(!polishOpen)}
          >
            <Sparkles size={14} />
            {t("settings.polishing")}
            {polishOpen ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </button>
          {polishOpen && (
            <div className="settings-section-body">
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.polish_level")}</span>
                <div className="settings-field-control">
                  <select
                    className="settings-select"
                    value={config.polishLevel}
                    onChange={(e) => update("polishLevel", e.target.value)}
                  >
                    <option value="none">{t("settings.polish_none")}</option>
                    <option value="light">{t("settings.polish_light")}</option>
                    <option value="medium">{t("settings.polish_medium")}</option>
                    <option value="heavy">{t("settings.polish_heavy")}</option>
                  </select>
                </div>
              </div>
              <p className="settings-hint settings-hint--polish">{t("settings.polish_level_hint")}</p>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.api_url")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.polishApiBaseUrl}
                    onChange={(e) => update("polishApiBaseUrl", e.target.value)}
                    placeholder="https://api.openai.com"
                  />
                </div>
              </div>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.model")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.polishModel}
                    onChange={(e) => update("polishModel", e.target.value)}
                    placeholder="gpt-4o-mini"
                  />
                </div>
              </div>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.api_key")}</span>
                <div className="settings-field-control">
                  <input
                    type="password"
                    className="settings-input"
                    value={config.polisherApiKey}
                    onChange={(e) => update("polisherApiKey", e.target.value)}
                    placeholder="sk-..."
                  />
                </div>
              </div>
            </div>
          )}
        </section>

        {/* Appearance */}
        <section className="settings-section">
          <h3 className="settings-section-title">
            <Palette size={14} />
            {t("settings.appearance")}
          </h3>
          <p className="settings-section-lead">{t("settings.appearance_lead")}</p>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.theme")}</span>
            <div className="settings-field-control">
              <select
                className="settings-select"
                value={themePref}
                onChange={(e) => setTheme(e.target.value as ThemePref)}
              >
                <option value="system">{t("settings.theme_system")}</option>
                <option value="light">{t("settings.theme_light")}</option>
                <option value="dark">{t("settings.theme_dark")}</option>
              </select>
            </div>
          </div>
        </section>

        {/* UI language */}
        <section className="settings-section">
          <h3 className="settings-section-title">
            <Globe size={14} />
            {t("settings.gui_language")}
          </h3>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.gui_language")}</span>
            <div className="settings-field-control">
              <select
                className="settings-select"
                value={config.guiLanguage}
                onChange={(e) => update("guiLanguage", e.target.value)}
              >
                <option value="zh">中文</option>
                <option value="en">English</option>
              </select>
            </div>
          </div>
        </section>

        <section className="settings-section">
          <h3 className="settings-section-title">
            <Sparkles size={14} />
            {t("settings.about")}
          </h3>
          <div className="settings-about-brand">
            <img src="/altgo-logo.svg" alt="" width={40} height={40} className="settings-about-logo" />
            <p className="settings-about-tagline">{t("settings.about_tagline")}</p>
          </div>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.version")}</span>
            <div className="settings-field-control">
              <span className="settings-muted">2.1.0</span>
            </div>
          </div>
        </section>

        <div className="settings-save-row">
          <p className="settings-save-hint">{t("settings.restart_hint")}</p>
          {message === "saved" && (
            <span className="settings-save-msg settings-save-msg--ok">
              <Check size={12} /> {t("settings.saved")}
            </span>
          )}
          {message && message !== "saved" && (
            <span className="settings-save-msg settings-save-msg--err">{message}</span>
          )}
          <button
            type="button"
            className="settings-btn settings-btn-primary"
            onClick={save}
            disabled={saving}
          >
            <Save size={13} />
            {saving ? t("settings.saving") : t("settings.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
