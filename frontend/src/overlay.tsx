import { createRoot } from "react-dom/client";
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import "./overlay.css";

function Overlay() {
  const [status, setStatus] = useState("idle");

  useEffect(() => {
    const unlisten = listen<string>("pipeline-status", (event) => {
      setStatus(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (status === "idle" || status === "stopped") return null;

  return (
    <div className="island">
      {status === "recording" && (
        <>
          <span className="pulse" />
          <span className="label">录音中...</span>
        </>
      )}
      {status === "processing" && (
        <>
          <span className="spinner" />
          <span className="label">处理中...</span>
        </>
      )}
      {status === "done" && (
        <>
          <span className="check">✓</span>
          <span className="label">完成</span>
        </>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
