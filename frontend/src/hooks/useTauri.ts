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
    let timer: number | null = null;
    const unlisten = listen<string>("transcription-result", (event) => {
      setText(event.payload);
      if (timer !== null) {
        clearTimeout(timer);
      }
      timer = window.setTimeout(() => setText(null), 5000);
    });
    return () => {
      unlisten.then((fn) => fn());
      if (timer !== null) {
        clearTimeout(timer);
      }
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

/** Linux: `evtest` or `xinput` — set when the voice pipeline starts. */
export function useKeyListenerBackend(): string | null {
  const [backend, setBackend] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<string>("key-listener-backend", (event) => {
      setBackend(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return backend;
}

/** 转写/润色阶段进度（`fraction` 为 null 时不确定进度，如云端 API 或未解析到 whisper-cli 输出）。 */
export function useTranscriptionProgress(): {
  phase: string;
  fraction: number | null;
} | null {
  const [progress, setProgress] = useState<{
    phase: string;
    fraction: number | null;
  } | null>(null);

  useEffect(() => {
    let active = true;
    const unlistenProgress = listen<{
      phase: string;
      fraction: number | null;
    }>("transcription-progress", (event) => {
      if (active) {
        setProgress(event.payload);
      }
    });
    const unlistenStatus = listen<string>("pipeline-status", (event) => {
      if (!active) return;
      if (event.payload !== "processing") {
        setProgress(null);
      }
    });
    return () => {
      active = false;
      unlistenProgress.then((fn) => fn());
      unlistenStatus.then((fn) => fn());
    };
  }, []);

  return progress;
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
