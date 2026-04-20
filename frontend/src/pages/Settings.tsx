import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import { useModelDownloadProgress } from "../hooks/useTauri";
import { Save, Globe, Mic, Sparkles, Check, Download, Trash2 } from "lucide-react";
import "../styles/components.css";

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
  transcriber_api_key: string;
  polisher_api_key: string;
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
    invoke<ModelEntry[]>("list_models").then(setModels).catch(() => {});
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
    <div className="model-list">
      {error && <p className="input-error">{error}</p>}
      {models.map((m) => (
        <div key={m.name} className="model-row">
          <div className="model-info">
            <span className="model-name">{m.name}</span>
            <span className="model-desc">
              {m.description} &middot; {formatSize(m.sizeBytes)}
            </span>
          </div>
          <div className="model-actions">
            {m.downloaded ? (
              <>
                <span className="model-badge">
                  <Check size={10} />
                  {t("settings.model_downloaded")}
                </span>
                <button
                  className="settings-btn settings-btn-sm settings-btn-danger"
                  onClick={() => handleDelete(m.name)}
                >
                  <Trash2 size={11} />
                  {t("settings.delete_model")}
                </button>
              </>
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
                className="settings-btn settings-btn-sm settings-btn-primary"
                onClick={() => handleDownload(m.name)}
                disabled={downloading !== null}
              >
                <Download size={11} />
                {t("settings.download_model")}
              </button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

export default function Settings() {
  const { t, setLang } = useTranslation();
  const [config, setConfig] = useState<Config | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<"saved" | string>("");

  useEffect(() => {
    invoke<Config>("get_config").then(setConfig).catch(() => {});
  }, []);

  if (!config) {
    return <div className="loading-container">{t("settings.loading")}</div>;
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
      setMessage("saved");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings-page">
      <div className="settings-form">

        {/* ── UI Language ── */}
        <div className="settings-section">
          <h3 className="settings-section-title">
            <Globe size={12} />
            {t("settings.gui_language")}
          </h3>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.gui_language")}</span>
            <div className="settings-field-control">
              <select
                className="settings-select"
                value={config.gui_language}
                onChange={(e) => update("gui_language", e.target.value)}
              >
                <option value="zh">中文</option>
                <option value="en">English</option>
              </select>
            </div>
          </div>
        </div>

        {/* ── Recording ── */}
        <div className="settings-section">
          <h3 className="settings-section-title">
            <Mic size={12} />
            {t("settings.recording")}
          </h3>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.key_name")}</span>
            <div className="settings-field-control">
              <input
                type="text"
                className="settings-input"
                value={config.key_name}
                onChange={(e) => update("key_name", e.target.value)}
                placeholder="Alt_R"
              />
            </div>
          </div>
        </div>

        {/* ── Transcription ── */}
        <div className="settings-section">
          <h3 className="settings-section-title">
            <Sparkles size={12} />
            {t("settings.transcription")}
          </h3>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.engine")}</span>
            <div className="settings-field-control">
              <select
                className="settings-select"
                value={config.engine}
                onChange={(e) => update("engine", e.target.value)}
              >
                <option value="api">{t("settings.engine_api")}</option>
                <option value="local">{t("settings.engine_local")}</option>
              </select>
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
          {config.engine === "api" ? (
            <>
              <div className="settings-field">
                <span className="settings-field-label-text">{t("settings.api_url")}</span>
                <div className="settings-field-control">
                  <input
                    type="text"
                    className="settings-input"
                    value={config.api_base_url}
                    onChange={(e) => update("api_base_url", e.target.value)}
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
                    value={config.transcriber_api_key || ""}
                    onChange={(e) => update("transcriber_api_key", e.target.value)}
                    placeholder="sk-..."
                  />
                </div>
              </div>
            </>
          ) : (
            <div className="settings-field">
              <span className="settings-field-label-text">{t("settings.model")}</span>
              <div className="settings-field-control">
                <input
                  type="text"
                  className="settings-input"
                  value={config.model}
                  onChange={(e) => update("model", e.target.value)}
                  placeholder="~/models/whisper.bin"
                />
              </div>
            </div>
          )}
        </div>

        {/* ── Model Management ── */}
        <div className="settings-section">
          <h3 className="settings-section-title">
            <Download size={12} />
            {t("settings.model_management")}
          </h3>
          <ModelSection t={t} />
        </div>

        {/* ── Polishing ── */}
        <div className="settings-section">
          <h3 className="settings-section-title">
            <Sparkles size={12} />
            {t("settings.polishing")}
          </h3>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.polish_level")}</span>
            <div className="settings-field-control">
              <select
                className="settings-select"
                value={config.polish_level}
                onChange={(e) => update("polish_level", e.target.value)}
              >
                <option value="none">{t("settings.polish_none")}</option>
                <option value="light">{t("settings.polish_light")}</option>
                <option value="medium">{t("settings.polish_medium")}</option>
                <option value="heavy">{t("settings.polish_heavy")}</option>
              </select>
            </div>
          </div>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.api_url")}</span>
            <div className="settings-field-control">
              <input
                type="text"
                className="settings-input"
                value={config.polish_api_base_url}
                onChange={(e) => update("polish_api_base_url", e.target.value)}
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
                value={config.polish_model}
                onChange={(e) => update("polish_model", e.target.value)}
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
                value={config.polisher_api_key || ""}
                onChange={(e) => update("polisher_api_key", e.target.value)}
                placeholder="sk-..."
              />
            </div>
          </div>
        </div>

        {/* ── About ── */}
        <div className="settings-section">
          <h3 className="settings-section-title">
            <Sparkles size={12} />
            {t("settings.about")}
          </h3>
          <div className="settings-field">
            <span className="settings-field-label-text">{t("settings.version")}</span>
            <div className="settings-field-control">
              <span style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)' }}>1.4.0</span>
            </div>
          </div>
        </div>

        {/* ── Save ── */}
        <div className="settings-save-row">
          {message === "saved" && (
            <span className="settings-save-msg settings-save-msg--ok">
              <Check size={12} /> {t("settings.saved")}
            </span>
          )}
          {message && message !== "saved" && (
            <span className="settings-save-msg settings-save-msg--err">
              {message}
            </span>
          )}
          <button
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
