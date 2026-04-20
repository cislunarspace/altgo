import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import { useModelDownloadProgress } from "../hooks/useTauri";

interface Config {
  key_name: string;
  language: string;
  engine: string;
  model: string;
  api_base_url: string;
  polish_level: string;
  polish_model: string;
  polish_api_base_url: string;
  gui_language: string;
}

interface ModelEntry {
  name: string;
  filename: string;
  sizeBytes: number;
  description: string;
  downloaded: boolean;
}

function formatSize(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

function ModelSection({ t }: { t: (k: string) => string }) {
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [error, setError] = useState("");
  const progress = useModelDownloadProgress();

  useEffect(() => {
    invoke<ModelEntry[]>("list_models").then(setModels).catch(console.error);
  }, []);

  const handleDownload = async (name: string) => {
    setDownloading(name);
    setError("");
    try {
      await invoke("download_model", { name });
      const updated = await invoke<ModelEntry[]>("list_models");
      setModels(updated);
    } catch (e) {
      setError(String(e));
    } finally {
      setDownloading(null);
    }
  };

  const handleDelete = async (name: string) => {
    try {
      await invoke("delete_model", { name });
      const updated = await invoke<ModelEntry[]>("list_models");
      setModels(updated);
    } catch (e) {
      setError(String(e));
    }
  };

  const downloadingProgress =
    downloading && progress.name === downloading
      ? Math.round((progress.downloaded / progress.total) * 100)
      : 0;

  return (
    <div className="settings-section">
      <h3 className="section-label">{t("settings.models")}</h3>
      {error && <p className="settings-error">{error}</p>}
      <div className="model-list">
        {models.map((m) => (
          <div key={m.name} className="model-row">
            <div className="model-info">
              <span className="model-name">{m.name}</span>
              <span className="model-desc">
                {m.description} · {formatSize(m.sizeBytes)}
              </span>
            </div>
            <div className="model-actions">
              {m.downloaded ? (
                <div className="model-downloaded">
                  <span className="badge badge-ok">✓</span>
                  <button
                    className="btn btn-sm btn-danger"
                    onClick={() => handleDelete(m.name)}
                  >
                    {t("settings.model_delete")}
                  </button>
                </div>
              ) : downloading === m.name ? (
                <div className="model-progress">
                  <div className="progress-bar">
                    <div
                      className="progress-fill"
                      style={{ width: `${downloadingProgress}%` }}
                    />
                  </div>
                  <span className="progress-text">{downloadingProgress}%</span>
                </div>
              ) : (
                <button
                  className="btn btn-sm btn-primary"
                  onClick={() => handleDownload(m.name)}
                  disabled={downloading !== null}
                >
                  {t("settings.model_download")}
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function Settings() {
  const { t, setLang } = useTranslation();
  const [config, setConfig] = useState<Config | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    invoke<Config>("get_config").then(setConfig).catch(console.error);
  }, []);

  if (!config) {
    return <div className="settings-loading">{t("settings.loading")}</div>;
  }

  const update = (key: keyof Config, value: string) => {
    setConfig((prev) => (prev ? { ...prev, [key]: value } : prev));
  };

  const save = async () => {
    setSaving(true);
    setMessage("");
    try {
      await invoke("save_config", {
        req: {
          keyName: config.key_name,
          language: config.language,
          engine: config.engine,
          model: config.model,
          apiBaseUrl: config.api_base_url,
          polishLevel: config.polish_level,
          polishModel: config.polish_model,
          polishApiBaseUrl: config.polish_api_base_url,
          guiLanguage: config.gui_language,
        },
      });
      setLang(config.gui_language);
      setMessage(t("settings.saved"));
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings">
      <h2 className="settings-title">{t("settings.title")}</h2>

      <div className="settings-section">
        <h3 className="section-label">{t("settings.gui_language")}</h3>
        <select
          value={config.gui_language}
          onChange={(e) => update("gui_language", e.target.value)}
          className="input"
        >
          <option value="zh">中文</option>
          <option value="en">English</option>
        </select>
      </div>

      <div className="settings-section">
        <h3 className="section-label">{t("settings.recording")}</h3>
        <label className="field">
          <span>{t("settings.key_name")}</span>
          <select
            className="input"
            value={config.key_name}
            onChange={(e) => update("key_name", e.target.value)}
          >
            <option value="ISO_Level3_Shift">Right Alt</option>
            <option value="Alt_L">Left Alt</option>
            <option value="Super_L">Left Win</option>
            <option value="Super_R">Right Win</option>
            <option value="Control_R">Right Ctrl</option>
            <option value="Shift_R">Right Shift</option>
          </select>
        </label>
      </div>

      <div className="settings-section">
        <h3 className="section-label">{t("settings.transcription")}</h3>
        <label className="field">
          <span>{t("settings.engine")}</span>
          <select
            className="input"
            value={config.engine}
            onChange={(e) => update("engine", e.target.value)}
          >
            <option value="local">{t("settings.engine_local")}</option>
            <option value="api">{t("settings.engine_api")}</option>
          </select>
        </label>
        <label className="field">
          <span>{t("settings.language")}</span>
          <select
            className="input"
            value={config.language}
            onChange={(e) => update("language", e.target.value)}
          >
            <option value="zh">中文</option>
            <option value="en">English</option>
            <option value="ja">日本語</option>
            <option value="ko">한국어</option>
            <option value="auto">Auto</option>
          </select>
        </label>
        {config.engine === "api" ? (
          <>
            <label className="field">
              <span>{t("settings.api_url")}</span>
              <input
                className="input"
                value={config.api_base_url}
                onChange={(e) => update("api_base_url", e.target.value)}
                placeholder="https://api.openai.com"
              />
            </label>
            <label className="field">
              <span>{t("settings.model")}</span>
              <input
                className="input"
                value={config.model}
                onChange={(e) => update("model", e.target.value)}
                placeholder="whisper-1"
              />
            </label>
          </>
        ) : (
          <>
            <ModelSection t={t} />
            <label className="field">
              <span>{t("settings.active_model")}</span>
              <input
                className="input"
                value={config.model}
                onChange={(e) => update("model", e.target.value)}
                placeholder={t("settings.active_model_placeholder")}
              />
            </label>
            <p className="field-hint">{t("settings.active_model_hint")}</p>
          </>
        )}
      </div>

      <div className="settings-section">
        <h3 className="section-label">{t("settings.polishing")}</h3>
        <label className="field">
          <span>{t("settings.polish_level")}</span>
          <select
            className="input"
            value={config.polish_level}
            onChange={(e) => update("polish_level", e.target.value)}
          >
            <option value="none">{t("settings.polish_none")}</option>
            <option value="light">{t("settings.polish_light")}</option>
            <option value="medium">{t("settings.polish_medium")}</option>
            <option value="heavy">{t("settings.polish_heavy")}</option>
          </select>
        </label>
        {config.polish_level !== "none" && (
          <>
            <label className="field">
              <span>{t("settings.api_url")}</span>
              <input
                className="input"
                value={config.polish_api_base_url}
                onChange={(e) => update("polish_api_base_url", e.target.value)}
                placeholder="https://api.openai.com"
              />
            </label>
            <label className="field">
              <span>{t("settings.model")}</span>
              <input
                className="input"
                value={config.polish_model}
                onChange={(e) => update("polish_model", e.target.value)}
                placeholder="gpt-4o-mini"
              />
            </label>
          </>
        )}
      </div>

      <div className="settings-actions">
        <button className="btn btn-primary" onClick={save} disabled={saving}>
          {saving ? t("settings.saving") : t("settings.save")}
        </button>
        {message && <p className="settings-message">{message}</p>}
      </div>

      <p className="settings-hint">{t("settings.restart_hint")}</p>
    </div>
  );
}
