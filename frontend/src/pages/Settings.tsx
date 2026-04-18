import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import { Save, Globe, Mic, Sparkles, Check, Download, Trash2 } from "lucide-react";
import "../styles/components.css";

// Config interface - keep same as before
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
  // API keys (not in config returned to frontend, use empty string)
  transcriber_api_key: string;
  polisher_api_key: string;
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
        {/* Section 1: Basic Settings */}
        <div className="settings-form-section">
          <h3 className="settings-form-section-title">
            <Globe size={14} />
            {t("settings.gui_language")}
          </h3>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.gui_language")}</span>
            <div className="settings-form-control">
              <select
                className="settings-form-select"
                value={config.gui_language}
                onChange={(e) => update("gui_language", e.target.value)}
              >
                <option value="zh">中文</option>
                <option value="en">English</option>
              </select>
            </div>
          </div>
        </div>

        {/* Section 2: Recording */}
        <div className="settings-form-section">
          <h3 className="settings-form-section-title">
            <Mic size={14} />
            {t("settings.recording")}
          </h3>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.key_name")}</span>
            <div className="settings-form-control">
              <input
                type="text"
                className="settings-form-input"
                value={config.key_name}
                onChange={(e) => update("key_name", e.target.value)}
                placeholder="Alt_R"
              />
            </div>
          </div>
        </div>

        {/* Section 3: Transcription */}
        <div className="settings-form-section">
          <h3 className="settings-form-section-title">
            <Sparkles size={14} />
            {t("settings.transcription")}
          </h3>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.engine")}</span>
            <div className="settings-form-control">
              <select
                className="settings-form-select"
                value={config.engine}
                onChange={(e) => update("engine", e.target.value)}
              >
                <option value="api">{t("settings.engine_api")}</option>
                <option value="local">{t("settings.engine_local")}</option>
              </select>
            </div>
          </div>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.language")}</span>
            <div className="settings-form-control">
              <input
                type="text"
                className="settings-form-input"
                value={config.language}
                onChange={(e) => update("language", e.target.value)}
                placeholder="zh"
              />
            </div>
          </div>
          {config.engine === "api" ? (
            <>
              <div className="settings-form-row">
                <span className="settings-form-label">{t("settings.api_url")}</span>
                <div className="settings-form-control">
                  <input
                    type="text"
                    className="settings-form-input"
                    value={config.api_base_url}
                    onChange={(e) => update("api_base_url", e.target.value)}
                    placeholder="https://api.openai.com"
                  />
                </div>
              </div>
              <div className="settings-form-row">
                <span className="settings-form-label">{t("settings.api_key")}</span>
                <div className="settings-form-control">
                  <input
                    type="password"
                    className="settings-form-input"
                    value={config.transcriber_api_key || ""}
                    onChange={(e) => update("transcriber_api_key", e.target.value)}
                    placeholder="sk-..."
                  />
                </div>
              </div>
            </>
          ) : (
            <div className="settings-form-row">
              <span className="settings-form-label">{t("settings.model_path")}</span>
              <div className="settings-form-control">
                <input
                  type="text"
                  className="settings-form-input"
                  value={config.model}
                  onChange={(e) => update("model", e.target.value)}
                  placeholder="~/models/whisper.bin"
                />
              </div>
            </div>
          )}
        </div>

        {/* Section 4: Model Management */}
        <div className="settings-form-section">
          <h3 className="settings-form-section-title">
            <Download size={14} />
            {"Model Management"}
          </h3>
          <div className="settings-form-row">
            <span className="settings-form-label">{"Current Model"}</span>
            <div className="settings-form-control">
              <span style={{ color: '#888', fontSize: '13px' }}>
                {config.model || "Not configured"}
              </span>
            </div>
          </div>
          <div className="settings-form-row">
            <span className="settings-form-label">{"Status"}</span>
            <div className="settings-form-control">
              <span style={{ color: config.model ? '#22c55e' : '#888', fontSize: '13px' }}>
                {config.model ? "Downloaded" : "Not downloaded"}
              </span>
            </div>
          </div>
          <div className="settings-form-row">
            <span className="settings-form-label"></span>
            <div className="settings-form-control" style={{ gap: '8px', justifyContent: 'flex-end' }}>
              <button className="settings-form-btn settings-form-btn-primary">
                <Download size={12} /> {"Download Model"}
              </button>
              <button className="settings-form-btn" disabled={!config.model}>
                <Trash2 size={12} /> {"Delete"}
              </button>
            </div>
          </div>
        </div>

        {/* Section 5: Polishing */}
        <div className="settings-form-section">
          <h3 className="settings-form-section-title">
            <Sparkles size={14} />
            {t("settings.polishing")}
          </h3>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.polish_level")}</span>
            <div className="settings-form-control">
              <select
                className="settings-form-select"
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
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.api_url")}</span>
            <div className="settings-form-control">
              <input
                type="text"
                className="settings-form-input"
                value={config.polish_api_base_url}
                onChange={(e) => update("polish_api_base_url", e.target.value)}
                placeholder="https://api.openai.com"
              />
            </div>
          </div>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.model")}</span>
            <div className="settings-form-control">
              <input
                type="text"
                className="settings-form-input"
                value={config.polish_model}
                onChange={(e) => update("polish_model", e.target.value)}
                placeholder="gpt-4o-mini"
              />
            </div>
          </div>
          <div className="settings-form-row">
            <span className="settings-form-label">{t("settings.api_key")}</span>
            <div className="settings-form-control">
              <input
                type="password"
                className="settings-form-input"
                value={config.polisher_api_key || ""}
                onChange={(e) => update("polisher_api_key", e.target.value)}
                placeholder="sk-..."
              />
            </div>
          </div>
        </div>

        {/* Section 6: About */}
        <div className="settings-form-section">
          <h3 className="settings-form-section-title">
            <Sparkles size={14} />
            {"About"}
          </h3>
          <div className="settings-form-row">
            <span className="settings-form-label">{"Version"}</span>
            <div className="settings-form-control">
              <span style={{ color: '#888', fontSize: '13px' }}>1.4.0</span>
            </div>
          </div>
          <div className="settings-form-row">
            <span className="settings-form-label"></span>
            <div className="settings-form-control">
              <button className="settings-form-btn">
                {"Check for Updates"}
              </button>
            </div>
          </div>
        </div>

        {/* Save Button */}
        <div className="settings-form-row" style={{ marginTop: '16px', borderTop: '1px solid #333', paddingTop: '16px' }}>
          <span className="settings-form-label"></span>
          <div className="settings-form-control">
            <button
              className="settings-form-btn settings-form-btn-primary"
              onClick={save}
              disabled={saving}
            >
              <Save size={12} />
              {saving ? t("settings.saving") : t("settings.save")}
            </button>
            {message === "saved" && (
              <span style={{ color: '#22c55e', fontSize: '13px', marginLeft: '12px' }}>
                <Check size={12} /> {t("settings.saved")}
              </span>
            )}
            {message && message !== "saved" && (
              <span style={{ color: '#ef4444', fontSize: '13px', marginLeft: '12px' }}>
                {message}
              </span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
