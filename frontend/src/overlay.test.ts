import { describe, it, expect, vi } from "vitest";

// Mock 模块以阻止 overlay.tsx 的副作用执行。
vi.mock("react-dom/client", () => ({ createRoot: () => ({ render: vi.fn() }) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./i18n", () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock("./theme", () => ({
  applyThemeToDocument: vi.fn(),
  getThemePref: vi.fn(),
  installThemeListeners: vi.fn(() => () => {}),
}));

import { computePhaseTransition } from "./overlay";

describe("computePhaseTransition", () => {
  // ---- hidden/null → visible：直接 show ----

  it("null → recording：直接显示", () => {
    expect(computePhaseTransition(null, "recording")).toEqual({
      action: "show",
      phase: "recording",
    });
  });

  it("hidden → processing：直接显示", () => {
    expect(computePhaseTransition("hidden", "processing")).toEqual({
      action: "show",
      phase: "processing",
    });
  });

  it("null → done：直接显示", () => {
    expect(computePhaseTransition(null, "done")).toEqual({
      action: "show",
      phase: "done",
    });
  });

  it("hidden → recording：直接显示", () => {
    expect(computePhaseTransition("hidden", "recording")).toEqual({
      action: "show",
      phase: "recording",
    });
  });

  // ---- visible → hidden：播放退出动画 ----

  it("recording → hidden：退出动画", () => {
    expect(computePhaseTransition("recording", "hidden")).toEqual({
      action: "exit",
    });
  });

  it("processing → hidden：退出动画", () => {
    expect(computePhaseTransition("processing", "hidden")).toEqual({
      action: "exit",
    });
  });

  it("done → hidden：退出动画", () => {
    expect(computePhaseTransition("done", "hidden")).toEqual({
      action: "exit",
    });
  });

  // ---- visible → 不同 visible：crossfade ----

  it("recording → processing：crossfade", () => {
    const result = computePhaseTransition("recording", "processing");
    expect(result.action).toBe("crossfade");
    expect(result.exitPhase).toBe("recording");
    expect(result.enterPhase).toBe("processing");
    expect(result.delay).toBe(180);
  });

  it("processing → done：crossfade", () => {
    const result = computePhaseTransition("processing", "done");
    expect(result.action).toBe("crossfade");
    expect(result.exitPhase).toBe("processing");
    expect(result.enterPhase).toBe("done");
    expect(result.delay).toBe(180);
  });

  it("recording → done：crossfade", () => {
    const result = computePhaseTransition("recording", "done");
    expect(result.action).toBe("crossfade");
    expect(result.exitPhase).toBe("recording");
    expect(result.enterPhase).toBe("done");
  });

  it("done → recording：crossfade", () => {
    const result = computePhaseTransition("done", "recording");
    expect(result.action).toBe("crossfade");
    expect(result.exitPhase).toBe("done");
    expect(result.enterPhase).toBe("recording");
  });

  // ---- visible → 相同 visible：不动作 ----

  it("recording → recording：不动作", () => {
    expect(computePhaseTransition("recording", "recording")).toEqual({
      action: "none",
    });
  });

  it("processing → processing：不动作", () => {
    expect(computePhaseTransition("processing", "processing")).toEqual({
      action: "none",
    });
  });

  it("done → done：不动作", () => {
    expect(computePhaseTransition("done", "done")).toEqual({
      action: "none",
    });
  });

  // ---- hidden → hidden：清除（与 null→hidden 行为一致） ----

  it("hidden → hidden：清除", () => {
    expect(computePhaseTransition("hidden", "hidden")).toEqual({
      action: "show",
      phase: null,
    });
  });

  // ---- null → hidden：清除 ----

  it("null → hidden：显示 null（清除）", () => {
    expect(computePhaseTransition(null, "hidden")).toEqual({
      action: "show",
      phase: null,
    });
  });
});
