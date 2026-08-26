import { createRoot } from "react-dom/client";
import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Copy, X, Check } from "lucide-react";
import { useTranslation } from "./i18n";
import { copyToClipboard } from "./utils/clipboard";
import { applyThemeToDocument, getThemePref, installThemeListeners } from "./theme";
import "./styles/overlay-base.css";
import "./styles/motion.css";
import "./overlay.css";

/** Overlay state emitted from Rust OverlayManager. */
interface OverlayState {
  phase: "recording" | "processing" | "done" | "hidden";
}

export type Phase = "recording" | "processing" | "done" | "hidden" | null;

export const CROSSFADE_DURATION_MS = 180;

/** 电平轨迹：采样间隔与窗口长度（10 秒 × 10 帧/秒 = 100 帧）。 */
export const TRACE_SAMPLE_INTERVAL_MS = 100;
export const TRACE_MAX_FRAMES = 100;

/** 把 0.0~1.0 的电平映射为轨迹条高度（px）。 */
export function traceBarHeight(level: number): number {
  const clamped = Math.min(1, Math.max(0, level));
  return 3 + clamped * 17;
}

export interface PhaseTransitionResult {
  action: "show" | "crossfade" | "exit" | "none";
  /** show: phase to display; exit→hidden: null to clear after animation */
  phase?: Phase;
  /** crossfade: current phase to keep during exit animation */
  exitPhase?: Phase;
  /** crossfade: phase to enter after delay */
  enterPhase?: Phase;
  /** crossfade: delay in ms before entering new phase */
  delay?: number;
}

/**
 * 纯函数：根据上一相位和新相位，决定下一步动作。
 *
 * - hidden/null → visible: 直接显示（show）
 * - visible → hidden: 播放退出动画（exit）
 * - visible → 不同 visible: crossfade（先 exit，延迟后切换）
 * - visible → 相同 visible: 不动作（none）
 * - hidden → hidden: 清除（show null）
 * - null → hidden: 显示 null（清除状态，show）
 */
export function computePhaseTransition(
  prevPhase: Phase,
  newPhase: "recording" | "processing" | "done" | "hidden"
): PhaseTransitionResult {
  // 进入隐藏状态
  if (newPhase === "hidden") {
    if (prevPhase !== null && prevPhase !== "hidden") {
      return { action: "exit" };
    }
    // hidden→hidden 或 null→hidden：清除
    return { action: "show", phase: null };
  }

  // 进入可见状态
  if (prevPhase === null || prevPhase === "hidden") {
    return { action: "show", phase: newPhase };
  }
  if (prevPhase === newPhase) {
    return { action: "none" };
  }
  // 可见→不同可见：crossfade
  return {
    action: "crossfade",
    exitPhase: prevPhase,
    enterPhase: newPhase,
    delay: CROSSFADE_DURATION_MS,
  };
}

export function Overlay() {
  const { t } = useTranslation();

  // Current visual phase — driven entirely by overlay-state event from Rust.
  const [phase, setPhase] = useState<string | null>(null);

  // Transcription result text (shown in done phase).
  const [result, setResult] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // 润色失败原因（done 阶段在文本下方提示已回退原文）。
  const [polishError, setPolishError] = useState<string | null>(null);

  // Progress info during processing.
  const [txProgress, setTxProgress] = useState<{
    phase: string;
    fraction: number | null;
  } | null>(null);

  // 电平轨迹：录音期间按 TRACE_SAMPLE_INTERVAL_MS 采样的电平历史（最多
  // TRACE_MAX_FRAMES 帧）。松开后冻结（processing 不采样），结果出现即清空。
  const [levelTrace, setLevelTrace] = useState<number[]>([]);
  const lastSampleAtRef = useRef(0);

  // Whether we are in an exit transition (CSS class toggle).
  const [isExiting, setIsExiting] = useState(false);
  // Crossfade（相位间切换）只做淡出，不带退出位移；exit（隐藏）才滑出。
  const [isCrossfading, setIsCrossfading] = useState(false);

  // Track previous phase for transition direction.
  const prevPhaseRef = useRef<Phase>(null);
  const crossfadeTimerRef = useRef<number | null>(null);

  useEffect(() => {
    applyThemeToDocument(getThemePref());
    return installThemeListeners(() => applyThemeToDocument(getThemePref()));
  }, []);

  useEffect(() => {
    let active = true;

    const clearCrossfadeTimer = () => {
      if (crossfadeTimerRef.current !== null) {
        window.clearTimeout(crossfadeTimerRef.current);
        crossfadeTimerRef.current = null;
      }
    };

    const unlistenState = listen<OverlayState>("overlay-state", (event) => {
      if (!active) return;
      clearCrossfadeTimer();
      const newPhase = event.payload.phase;
      const prev = prevPhaseRef.current;
      const transition = computePhaseTransition(prev, newPhase);

      switch (transition.action) {
        case "show":
          setIsExiting(false);
          setIsCrossfading(false);
          setPhase(transition.phase!);
          break;
        case "exit":
          setIsCrossfading(false);
          setIsExiting(true);
          break;
        case "crossfade":
          // 进入可见相位时清除转写状态
          setTxProgress(null);
          if (newPhase !== "done") {
            setResult(null);
            setCopied(false);
            setPolishError(null);
          }
          if (newPhase === "recording") {
            // 新一轮录音：轨迹重新累积（不存在 processing→recording 路径，
            // 进入 recording 即代表上一轮已结束）。
            setLevelTrace([]);
          } else if (newPhase === "done") {
            // 结果文本已出，轨迹完成使命随 crossfade 退场。
            setLevelTrace([]);
          }
          setIsCrossfading(true);
          setIsExiting(true);
          prevPhaseRef.current = transition.enterPhase ?? null;
          requestAnimationFrame(() => {
            crossfadeTimerRef.current = window.setTimeout(() => {
              if (!active) return;
              crossfadeTimerRef.current = null;
              setIsExiting(false);
              setIsCrossfading(false);
              setPhase(transition.enterPhase!);
            }, transition.delay);
          });
          return;
        case "none":
          return;
      }

      // show/exit 分支也清除转写状态（进入非 done 的可见相位时）
      if (newPhase !== "hidden" && newPhase !== "done") {
        setTxProgress(null);
        setResult(null);
        setCopied(false);
        setPolishError(null);
        if (newPhase === "recording") {
          setLevelTrace([]);
        }
      } else if (newPhase === "done") {
        setTxProgress(null);
        setLevelTrace([]);
      }
      prevPhaseRef.current = newPhase;
    });

    const unlistenAudioLevel = listen<number>("audio-level", (event) => {
      if (!active) return;
      const level = event.payload ?? 0;
      // 仅录音相位采样；processing 冻结轨迹，done/hidden 不采。
      if (prevPhaseRef.current !== "recording") return;
      const now = Date.now();
      if (now - lastSampleAtRef.current < TRACE_SAMPLE_INTERVAL_MS) return;
      lastSampleAtRef.current = now;
      setLevelTrace((prev) => {
        const next = [...prev, level];
        return next.length > TRACE_MAX_FRAMES
          ? next.slice(next.length - TRACE_MAX_FRAMES)
          : next;
      });
    });

    const unlistenResult = listen<string>("transcription-result", (event) => {
      if (!active) return;
      setResult(event.payload);
      setCopied(false);
    });

    const unlistenPolishFailed = listen<string>("polish-failed", (event) => {
      if (!active) return;
      setPolishError(event.payload);
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
      clearCrossfadeTimer();
      unlistenState.then((fn) => fn());
      unlistenResult.then((fn) => fn());
      unlistenPolishFailed.then((fn) => fn());
      unlistenTxProgress.then((fn) => fn());
      unlistenAudioLevel.then((fn) => fn());
    };
  }, []);

  // Listen for CSS transition end to fully clear hidden state.
  // 只响应容器自身的 transitionend；子元素（进度条、按钮等）的
  // transition 结束会冒泡上来，不得因此提前清除内容。
  const handleTransitionEnd = (event: React.TransitionEvent) => {
    if (event.target !== event.currentTarget) return;
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
      const ok = await copyToClipboard(result);
      if (ok) {
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
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

  const containerClass = `island-container ${
    isExiting ? (isCrossfading ? "island-crossfade" : "island-exit") : "island-enter"
  }`;

  // done 事件可能先于 transcription-result 到达；结果未到时继续显示
  // processing 视图，避免渲染出没有内容的空 island（闪烁）。
  const effectivePhase = phase === "done" && !result ? "processing" : phase;

  if (phase === "done" && result) {
    return (
      <div className={containerClass} onTransitionEnd={handleTransitionEnd}>
        <div className="island-result">
          <div className="done-indicator">
            <Check size={14} className="done-icon" strokeWidth={2.5} />
          </div>
          <div className="result-content">
            <span className="result-text">{result}</span>
            {polishError && (
              <span className="result-polish-error" title={polishError}>
                {t("overlay.polish_failed")}
              </span>
            )}
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
        {(effectivePhase === "recording" || effectivePhase === "processing") && (
          <div
            className={`level-trace ${effectivePhase === "processing" ? "level-trace--frozen" : ""}`}
            aria-hidden
          >
            {levelTrace.map((level, i) => (
              <div
                key={i}
                className="trace-bar"
                style={{ height: `${traceBarHeight(level).toFixed(1)}px` }}
              />
            ))}
          </div>
        )}
        {effectivePhase === "recording" && (
          <span className="label">{t("overlay.recording")}</span>
        )}
        {effectivePhase === "processing" && (
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
