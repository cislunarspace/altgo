import { createRoot } from "react-dom/client";
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "./i18n";
import "./overlay.css";

function Overlay() {
  const [status, setStatus] = useState("idle");
  const [result, setResult] = useState<string | null>(null);
  const { t } = useTranslation();

  useEffect(() => {
    const unlistenStatus = listen<string>("pipeline-status", (event) => {
      setStatus(event.payload);
      if (event.payload === "idle" || event.payload === "stopped") {
        setResult(null);
      }
    });
    const unlistenResult = listen<string>("transcription-result", (event) => {
      setResult(event.payload);
    });
    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenResult.then((fn) => fn());
    };
  }, []);

  if (status === "idle" || status === "stopped") return null;

  if (result && status === "done") {
    return (
      <div className="island island-result">
        <span className="check">✓</span>
        <span className="result-text">{result}</span>
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
      {status === "done" && (
        <>
          <span className="check">✓</span>
          <span className="label">{t("status.done")}</span>
        </>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
