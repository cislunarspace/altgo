import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";

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
          <input
            className="input"
            value={config.key_name}
            onChange={(e) => update("key_name", e.target.value)}
          />
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
            <option value="api">{t("settings.engine_api")}</option>
            <option value="local">{t("settings.engine_local")}</option>
          </select>
        </label>
        <label className="field">
          <span>{t("settings.language")}</span>
          <input
            className="input"
            value={config.language}
            onChange={(e) => update("language", e.target.value)}
          />
        </label>
        {config.engine === "api" ? (
          <>
            <label className="field">
              <span>{t("settings.api_url")}</span>
              <input
                className="input"
                value={config.api_base_url}
                onChange={(e) => update("api_base_url", e.target.value)}
              />
            </label>
          </>
        ) : (
          <label className="field">
            <span>{t("settings.model_path")}</span>
            <input
              className="input"
              value={config.model}
              onChange={(e) => update("model", e.target.value)}
            />
          </label>
        )}
        <label className="field">
          <span>{t("settings.model")}</span>
          <input
            className="input"
            value={config.model}
            onChange={(e) => update("model", e.target.value)}
          />
        </label>
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
        <label className="field">
          <span>{t("settings.api_url")}</span>
          <input
            className="input"
            value={config.polish_api_base_url}
            onChange={(e) => update("polish_api_base_url", e.target.value)}
          />
        </label>
        <label className="field">
          <span>{t("settings.model")}</span>
          <input
            className="input"
            value={config.polish_model}
            onChange={(e) => update("polish_model", e.target.value)}
          />
        </label>
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
