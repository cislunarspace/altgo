import { useStatus, useLatestTranscription, usePipelineError } from "../hooks/useTauri";
import { useTranslation } from "../i18n";

export default function Home() {
  const { t } = useTranslation();
  const status = useStatus();
  const transcription = useLatestTranscription();
  const error = usePipelineError();

  const statusConfig: Record<string, { emoji: string; color: string; text: string }> = {
    idle: { emoji: "🎤", color: "var(--text-muted)", text: t("status.idle") },
    recording: { emoji: "🔴", color: "var(--accent-red)", text: t("status.recording") },
    processing: { emoji: "⚙️", color: "var(--accent-yellow)", text: t("status.processing") },
    done: { emoji: "✅", color: "var(--accent-green)", text: t("status.done") },
  };

  const current = statusConfig[status] || statusConfig.idle;

  return (
    <div className="home">
      {error && (
        <div className="home-error">
          <span className="error-icon">⚠️</span>
          <p className="error-text">{error}</p>
        </div>
      )}
      {!transcription ? (
        <div className="home-idle">
          <span className="home-emoji" style={{ color: current.color }}>
            {current.emoji}
          </span>
          <p className="home-status" style={{ color: current.color }}>
            {current.text}
          </p>
          <p className="home-hint">{t("main.hint")}</p>
        </div>
      ) : (
        <div className="home-result">
          <p className="result-label">{t("main.result_label")}</p>
          <div className="result-box">
            <p className="result-text">{transcription}</p>
          </div>
          <p className="result-copied">{t("main.copied")}</p>
        </div>
      )}
    </div>
  );
}
