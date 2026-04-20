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

export function usePipelineError(): string | null {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<string>("pipeline-error", (event) => {
      setError(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return error;
}

export function useModelDownloadProgress(): {
  name: string | null;
  downloaded: number;
  total: number;
} {
  const [progress, setProgress] = useState<{
    name: string | null;
    downloaded: number;
    total: number;
  }>({ name: null, downloaded: 0, total: 0 });

  useEffect(() => {
    const unlisten = listen<{
      name: string;
      downloaded: number;
      total: number;
    }>("model-download-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return progress;
}
