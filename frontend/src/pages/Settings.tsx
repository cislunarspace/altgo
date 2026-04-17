import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import { Card } from "../components/ui/Card";
import { Input, Select } from "../components/ui/Input";
import { Button } from "../components/ui/Button";
import { Save, Globe, Mic, Sparkles, Check } from "lucide-react";
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
    return (
      <div className="loading-container">
        {t("settings.loading")}
      </div>
    );
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
      <h2 className="settings-title">
        <Sparkles size={20} color="var(--color-accent)" />
        {t("settings.title")}
      </h2>

      {/* Language Section */}
      <div className="settings-section">
        <Card padding="lg">
          <h3 className="settings-section-title">
            <Globe size={16} className="settings-section-icon" />
            {t("settings.gui_language")}
          </h3>
          <div className="settings-field-grid">
            <Select
              label={t("settings.gui_language")}
              value={config.gui_language}
              onChange={(e) => update("gui_language", e.target.value)}
            >
              <option value="zh">中文</option>
              <option value="en">English</option>
            </Select>
          </div>
        </Card>
      </div>

      {/* Recording Section */}
      <div className="settings-section">
        <Card padding="lg">
          <h3 className="settings-section-title">
            <Mic size={16} className="settings-section-icon" />
            {t("settings.recording")}
          </h3>
          <div className="settings-field-grid">
            <Input
              label={t("settings.key_name")}
              value={config.key_name}
              onChange={(e) => update("key_name", e.target.value)}
              placeholder="Alt_R"
            />
          </div>
        </Card>
      </div>

      {/* Transcription Section */}
      <div className="settings-section">
        <Card padding="lg">
          <h3 className="settings-section-title">
            <Sparkles size={16} className="settings-section-icon" />
            {t("settings.transcription")}
          </h3>
          <div className="settings-field-grid">
            <Select
              label={t("settings.engine")}
              value={config.engine}
              onChange={(e) => update("engine", e.target.value)}
            >
              <option value="api">{t("settings.engine_api")}</option>
              <option value="local">{t("settings.engine_local")}</option>
            </Select>
            <Input
              label={t("settings.language")}
              value={config.language}
              onChange={(e) => update("language", e.target.value)}
              placeholder="zh"
            />
            {config.engine === "api" ? (
              <Input
                label={t("settings.api_url")}
                value={config.api_base_url}
                onChange={(e) => update("api_base_url", e.target.value)}
                placeholder="https://api.openai.com"
              />
            ) : (
              <Input
                label={t("settings.model_path")}
                value={config.model}
                onChange={(e) => update("model", e.target.value)}
                placeholder="~/models/whisper.bin"
              />
            )}
            <Input
              label={t("settings.model")}
              value={config.model}
              onChange={(e) => update("model", e.target.value)}
              placeholder="whisper-1"
            />
          </div>
        </Card>
      </div>

      {/* Polishing Section */}
      <div className="settings-section">
        <Card padding="lg">
          <h3 className="settings-section-title">
            <Sparkles size={16} className="settings-section-icon" />
            {t("settings.polishing")}
          </h3>
          <div className="settings-field-grid">
            <Select
              label={t("settings.polish_level")}
              value={config.polish_level}
              onChange={(e) => update("polish_level", e.target.value)}
            >
              <option value="none">{t("settings.polish_none")}</option>
              <option value="light">{t("settings.polish_light")}</option>
              <option value="medium">{t("settings.polish_medium")}</option>
              <option value="heavy">{t("settings.polish_heavy")}</option>
            </Select>
            <Input
              label={t("settings.api_url")}
              value={config.polish_api_base_url}
              onChange={(e) => update("polish_api_base_url", e.target.value)}
              placeholder="https://api.openai.com"
            />
            <Input
              label={t("settings.model")}
              value={config.polish_model}
              onChange={(e) => update("polish_model", e.target.value)}
              placeholder="gpt-4o-mini"
            />
          </div>
        </Card>
      </div>

      <div className="settings-actions">
        <Button onClick={save} disabled={saving}>
          <Save size={16} />
          {saving ? t("settings.saving") : t("settings.save")}
        </Button>
        {message === "saved" && (
          <span className="settings-message">
            <Check size={16} />
            {t("settings.saved")}
          </span>
        )}
        {message && message !== "saved" && (
          <span className="settings-message error">
            {message}
          </span>
        )}
      </div>

      <p className="settings-hint">{t("settings.restart_hint")}</p>
    </div>
  );
}
