import {
  useStatus,
  useLatestTranscription,
  usePipelineError,
  useKeyListenerBackend,
} from "../hooks/useTauri";
import { useTranslation } from "../i18n";
import { StatusIndicator } from "../components/StatusIndicator";
import { Copy, Check } from "lucide-react";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "../styles/components.css";

export default function Home() {
  const { t } = useTranslation();
  const status = useStatus();
  const transcription = useLatestTranscription();
  const error = usePipelineError();
  const keyBackend = useKeyListenerBackend();
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    if (transcription) {
      try {
        await invoke("copy_text", { text: transcription });
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch {
        // Clipboard failed silently
      }
    }
  };

  const statusMap: Record<string, 'idle' | 'recording' | 'processing' | 'done'> = {
    idle: 'idle',
    recording: 'recording',
    processing: 'processing',
    done: 'done',
  };

  const mappedStatus = statusMap[status] || 'idle';

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
          <StatusIndicator status={mappedStatus} size="lg" />
          <p className="home-hint">{t("main.hint")}</p>
          {keyBackend && (
            <p className="home-key-backend">{t(`main.key_backend_${keyBackend}`)}</p>
          )}
        </div>
      ) : (
        <div className="home-result">
          <span className="home-result-label">{t("main.result_label")}</span>
          <div className="home-result-card">
            <p className="home-result-text">{transcription}</p>
          </div>
          <button
            className={`home-copy-btn ${copied ? 'copied' : ''}`}
            onClick={handleCopy}
          >
            {copied ? (
              <>
                <Check size={16} color="var(--color-accent-green)" />
                <span style={{ color: 'var(--color-accent-green)' }}>已复制</span>
              </>
            ) : (
              <>
                <Copy size={16} />
                <span>{t("main.copy")}</span>
              </>
            )}
          </button>
        </div>
      )}
    </div>
  );
}
