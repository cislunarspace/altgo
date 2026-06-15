import { useEffect, useState } from "react";
import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Subscribe to a Tauri event. Returns the latest payload (or `initial` if no event yet).
 *
 * The callback is captured in the effect closure, so changing it does not re-subscribe.
 * Use the returned `payload` to drive UI; if you need a derived transformation, do it
 * in the caller.
 */
export function useTauriEvent<T>(
  event: string,
  initial: T,
  callback?: EventCallback<T>,
): T {
  const [state, setState] = useState<T>(initial);

  useEffect(() => {
    let active = true;
    const unlistenPromise: Promise<UnlistenFn> = listen<T>(event, (event) => {
      if (!active) return;
      setState(event.payload);
      callback?.(event);
    });
    return () => {
      active = false;
      unlistenPromise.then((fn) => fn());
    };
    // callback is intentionally not a dep; consumers that need it can memoise.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [event]);

  return state;
}

export function useStatus(): string {
  return useTauriEvent<string>("pipeline-status", "idle");
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
  return useTauriEvent<string | null>("pipeline-error", null);
}

export function useKeyListenerBackend(): string | null {
  return useTauriEvent<string | null>("key-listener-backend", null);
}

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
      if (active) setProgress(event.payload);
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
  return useTauriEvent("model-download-progress", {
    name: null,
    downloaded: 0,
    total: 0,
  });
}
