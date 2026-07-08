import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { StatusIndicator } from "./StatusIndicator";

vi.mock("../i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("StatusIndicator", () => {
  it("renders idle state", () => {
    const { container } = render(<StatusIndicator status="idle" />);
    expect(container.querySelector(".status-indicator")).toBeTruthy();
    expect(container.querySelector(".status-label")?.textContent).toBe("status.idle");
  });

  it("renders recording state", () => {
    const { container } = render(<StatusIndicator status="recording" />);
    expect(container.querySelector(".status-label")?.textContent).toBe("status.recording");
  });

  it("renders processing state", () => {
    const { container } = render(<StatusIndicator status="processing" />);
    expect(container.querySelector(".spinning")).toBeTruthy();
  });

  it("applies custom size", () => {
    const { container } = render(<StatusIndicator status="idle" size="lg" />);
    const svg = container.querySelector("svg");
    expect(svg?.getAttribute("width")).toBe("96");
  });
});
