import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

export function useStatus(): string {
  const [status, setStatus] = useState("idle");

  useEffect(() => {
    const unlisten = listen<string>("pipeline-status", (event) => {
      setStatus(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return status;
}

export function useLatestTranscription(): string | null {
  const [text, setText] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<string>("transcription-result", (event) => {
      setText(event.payload);
      setTimeout(() => setText(null), 5000);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return text;
}
