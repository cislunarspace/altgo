import { createRoot } from "react-dom/client";
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Copy, X, Check } from "lucide-react";
import "./overlay.css";

function Overlay() {
  const [status, setStatus] = useState("idle");
  const [result, setResult] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

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

  if (result && status === "done") {
    return (
      <div className="island island-result">
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
                <span>已复制</span>
              </>
            ) : (
              <>
                <Copy size={13} />
                <span>复制</span>
              </>
            )}
          </button>
          <button className="btn-close" onClick={handleClose}>
            <X size={14} />
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="island">
      {status === "recording" && (
        <>
          <div className="recording-indicator">
            <div className="recording-glow" />
            <div className="recording-core" />
          </div>
          <span className="label">录音中...</span>
        </>
      )}
      {status === "processing" && (
        <>
          <div className="processing-indicator">
            <div className="processing-ring" />
          </div>
          <span className="label">处理中...</span>
        </>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
