import { createRoot } from "react-dom/client";
import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Copy, X, Check } from "lucide-react";
import { useOverlayTranslation } from "./i18n";
import { applyThemeToDocument, getThemePref, installThemeListeners } from "./theme";
import "./styles/overlay-base.css";
import "./styles/motion.css";
import "./overlay.css";

function Overlay() {
  const { t } = useOverlayTranslation();
  const [status, setStatus] = useState("idle");
  const [result, setResult] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [isExiting, setIsExiting] = useState(false);
  const [displayedStatus, setDisplayedStatus] = useState<string | null>(null);
  const prevStatusRef = useRef("idle");
  const exitTimerRef = useRef<number | null>(null);

  useEffect(() => {
    applyThemeToDocument(getThemePref());
    return installThemeListeners(() => applyThemeToDocument(getThemePref()));
  }, []);

  useEffect(() => {
    const unlistenStatus = listen<string>("pipeline-status", (event) => {
      const newStatus = event.payload;

      if (newStatus === "idle" || newStatus === "stopped") {
        // Transitioning to hidden state
        if (prevStatusRef.current === "idle" || prevStatusRef.current === "stopped") {
          // Already hidden, just update
          setStatus(newStatus);
        } else {
          // Was showing content, play exit animation
          setIsExiting(true);
          exitTimerRef.current = setTimeout(() => {
            setDisplayedStatus(null);
            setStatus(newStatus);
            setIsExiting(false);
          }, 150);
        }
      } else {
        // Transitioning to a visible state
        if (status === "idle" || status === "stopped") {
          // From hidden, just show directly
          setDisplayedStatus(newStatus);
          setStatus(newStatus);
        } else if (displayedStatus !== newStatus) {
          // Different visible state, crossfade
          setIsExiting(true);
          exitTimerRef.current = setTimeout(() => {
            setDisplayedStatus(newStatus);
            setIsExiting(false);
          }, 150);
        }
      }
      prevStatusRef.current = newStatus;
    });
    const unlistenResult = listen<string>("transcription-result", (event) => {
      setResult(event.payload);
      setCopied(false);
    });
    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenResult.then((fn) => fn());
      if (exitTimerRef.current !== null) {
        clearTimeout(exitTimerRef.current);
      }
    };
  }, [status, displayedStatus]);

  if (displayedStatus === null && !isExiting) return null;

  const handleCopy = async () => {
    if (result) {
      try {
        await invoke("copy_text", { text: result });
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
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

  if ((displayedStatus === "done" || status === "done") && result) {
    return (
      <div className={`island ${isExiting ? "island-exit" : ""}`}>
        <div className="island-result">
          <div className="done-indicator">
            <Check size={12} className="done-icon" />
          </div>
          <div className="result-content">
            <span className="result-text">{result}</span>
          </div>
          <div className="result-actions">
            <button
              className={`btn-copy ${copied ? "copied" : ""}`}
              onClick={handleCopy}
            >
              {copied ? (
                <>
                  <Check size={13} />
                  <span>{t("overlay.copied")}</span>
                </>
              ) : (
                <>
                  <Copy size={13} />
                  <span>{t("overlay.copy")}</span>
                </>
              )}
            </button>
            <button className="btn-close" onClick={handleClose}>
              <X size={14} />
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`island ${isExiting ? "island-exit" : ""}`}>
      {displayedStatus === "recording" && (
        <>
          <div className="recording-indicator">
            <div className="recording-glow" />
            <div className="recording-core" />
          </div>
          <span className="label">{t("overlay.recording")}</span>
        </>
      )}
      {displayedStatus === "processing" && (
        <>
          <div className="processing-indicator">
            <div className="processing-ring" />
          </div>
          <span className="label">{t("overlay.processing")}</span>
        </>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
