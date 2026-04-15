import { createRoot } from "react-dom/client";
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "./i18n";
import "./overlay.css";

function Overlay() {
  const [status, setStatus] = useState("idle");
  const [result, setResult] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const { t } = useTranslation();

  useEffect(() => {
    const unlistenStatus = listen<string>("pipeline-status", (event) => {
      setStatus(event.payload);
      if (event.payload === "idle" || event.payload === "stopped") {
        setResult(null);
        setCopied(false);
      }
    });
    const unlistenResult = listen<string>("transcription-result", (event) => {
      setResult(event.payload);
      setCopied(true);
    });
    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenResult.then((fn) => fn());
    };
  }, []);

  if (status === "idle" || status === "stopped") return null;

  const handleCopy = async () => {
    if (result) {
      try {
        await invoke("copy_text", { text: result });
        setCopied(true);
      } catch {
        // Clipboard write failed — ignore silently
      }
    }
  };

  const handleClose = async () => {
    try {
      await invoke("hide_overlay");
    } catch {
      // Overlay hide failed — ignore silently
    }
  };

  if (result && status === "done") {
    return (
      <div className="island island-result">
        <span className="check">✓</span>
        <span className="result-text">{result}</span>
        <div className="result-actions">
          <button className="btn-copy" onClick={handleCopy}>
            {copied ? "✓" : t("overlay.copy")}
          </button>
          <button className="btn-close" onClick={handleClose}>
            ✕
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="island">
      {status === "recording" && (
        <>
          <span className="pulse" />
          <span className="label">{t("overlay.recording")}</span>
        </>
      )}
      {status === "processing" && (
        <>
          <span className="spinner" />
          <span className="label">{t("overlay.processing")}</span>
        </>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
