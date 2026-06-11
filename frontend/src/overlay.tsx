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

/** Overlay state emitted from Rust OverlayManager. */
interface OverlayState {
  phase: "recording" | "processing" | "done" | "hidden";
}

function Overlay() {
  const { t } = useOverlayTranslation();

  // Current visual phase — driven entirely by overlay-state event from Rust.
  const [phase, setPhase] = useState<string | null>(null);

  // Transcription result text (shown in done phase).
  const [result, setResult] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Progress info during processing.
  const [txProgress, setTxProgress] = useState<{
    phase: string;
    fraction: number | null;
  } | null>(null);

  // Whether we are in an exit transition (CSS class toggle).
  const [isExiting, setIsExiting] = useState(false);

  // Track previous phase for transition direction.
  const prevPhaseRef = useRef<string | null>(null);

  useEffect(() => {
    applyThemeToDocument(getThemePref());
    return installThemeListeners(() => applyThemeToDocument(getThemePref()));
  }, []);

  useEffect(() => {
    let active = true;

    const unlistenState = listen<OverlayState>("overlay-state", (event) => {
      if (!active) return;
      const newPhase = event.payload.phase;
      const prev = prevPhaseRef.current;

      if (newPhase === "hidden") {
        // Play exit animation, then clear.
        if (prev !== null && prev !== "hidden") {
          setIsExiting(true);
        } else {
          setPhase(null);
          setIsExiting(false);
        }
      } else {
        // Entering a visible phase.
        setTxProgress(null);
        if (prev === null || prev === "hidden") {
          // From hidden — direct show.
          setIsExiting(false);
          setPhase(newPhase);
        } else if (prev !== newPhase) {
          // Crossfade between visible phases.
          setIsExiting(true);
          // Wait for CSS exit transition, then show new phase.
          // The 150ms matches CSS transition-duration.
          requestAnimationFrame(() => {
            const timer = window.setTimeout(() => {
              if (!active) return;
              setIsExiting(false);
              setPhase(newPhase);
            }, 150);
            // Store timer for cleanup.
            (unlistenState as any).__timer = timer;
          });
          return; // Don't update prevPhaseRef yet.
        }
      }
      prevPhaseRef.current = newPhase;
    });

    const unlistenResult = listen<string>("transcription-result", (event) => {
      if (!active) return;
      setResult(event.payload);
      setCopied(false);
    });

    const unlistenTxProgress = listen<{
      phase: string;
      fraction: number | null;
    }>("transcription-progress", (event) => {
      if (!active) return;
      setTxProgress(event.payload);
    });

    return () => {
      active = false;
      unlistenState.then((fn) => fn());
      unlistenResult.then((fn) => fn());
      unlistenTxProgress.then((fn) => fn());
    };
  }, []);

  // Listen for CSS transition end to fully clear hidden state.
  const handleTransitionEnd = () => {
    if (isExiting) {
      setIsExiting(false);
      if (prevPhaseRef.current === "hidden") {
        setPhase(null);
      }
    }
  };

  if (phase === null && !isExiting) return null;

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

  const containerClass = `island-container ${isExiting ? "island-exit" : "island-enter"}`;

  if (phase === "done" && result) {
    return (
      <div className={containerClass} onTransitionEnd={handleTransitionEnd}>
        <div className="island-result">
          <div className="done-indicator">
            <Check size={14} className="done-icon" strokeWidth={2.5} />
          </div>
          <div className="result-content">
            <span className="result-text">{result}</span>
          </div>
          <div className="result-actions">
            <button
              type="button"
              className={`btn-copy ${copied ? "copied" : ""}`}
              onClick={handleCopy}
              aria-label={copied ? t("overlay.copied") : t("overlay.copy")}
            >
              {copied ? (
                <>
                  <Check size={14} strokeWidth={2.5} />
                  <span>{t("overlay.copied")}</span>
                </>
              ) : (
                <>
                  <Copy size={14} strokeWidth={2} />
                  <span>{t("overlay.copy")}</span>
                </>
              )}
            </button>
            <button
              type="button"
              className="btn-close"
              onClick={handleClose}
              aria-label={t("overlay.close")}
            >
              <X size={14} strokeWidth={2.5} aria-hidden />
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={containerClass} onTransitionEnd={handleTransitionEnd}>
      <div className="island">
        {phase === "recording" && (
          <>
            <div className="recording-indicator">
              <div className="recording-glow" />
              <div className="recording-core" />
            </div>
            <span className="label">{t("overlay.recording")}</span>
          </>
        )}
        {phase === "processing" && (
          <div className="island-processing-inner">
            <div className="island-processing-row">
              <div className="processing-indicator">
                <div className="processing-ring" />
              </div>
              <span className="label">
                {txProgress?.phase === "polish"
                  ? t("overlay.polishing")
                  : t("overlay.transcribing")}
              </span>
            </div>
            <div className="overlay-tx-progress-track">
              <div
                className={`overlay-tx-progress-fill ${
                  txProgress?.fraction == null ? "indeterminate" : ""
                }`}
                style={
                  txProgress?.fraction != null
                    ? {
                        transform: `scaleX(${Math.min(
                          1,
                          Math.max(0, txProgress.fraction)
                        )})`,
                      }
                    : undefined
                }
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
