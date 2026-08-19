import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProviderPresetSelector } from "./ProviderPresetSelector";
import type { ProviderPreset } from "../config/modelPresets";

const preset: ProviderPreset = {
  name: "Example Provider",
  websiteUrl: "https://example.com",
  apiKeyUrl: "https://example.com/key",
  apiBaseUrl: "https://api.example.com/v1",
  category: "custom",
  modelTypes: ["polisher"],
  apiFormat: "openai",
  defaultModel: "example-model",
  models: [
    {
      model: "example-model",
      displayName: "Example Model",
      recommended: true,
    },
  ],
};

const t = (key: string) => key;
const baseProps = {
  presets: [preset],
  modelType: "polisher" as const,
  currentApiBaseUrl: "",
  currentModel: "",
  lang: "en",
  t,
  onSelect: vi.fn(),
};

describe("ProviderPresetSelector", () => {
  it("keeps the provider list closed until requested", () => {
    const { container } = render(<ProviderPresetSelector {...baseProps} />);

    expect(screen.getByText("settings.add_provider")).toBeTruthy();
    expect(container.querySelector(".provider-preset-dialog")).toBeNull();
  });

  it("opens the picker and selects a recommended model", () => {
    const onSelect = vi.fn();
    render(<ProviderPresetSelector {...baseProps} onSelect={onSelect} />);

    fireEvent.click(screen.getByText("settings.add_provider"));
    fireEvent.click(screen.getByText("Example Provider"));
    fireEvent.click(screen.getByText("Example Model"));

    expect(onSelect).toHaveBeenCalledWith(preset, preset.models[0]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("closes with Escape and restores focus to the trigger", () => {
    render(<ProviderPresetSelector {...baseProps} />);
    const trigger = screen.getByText("settings.add_provider").closest("button");

    fireEvent.click(trigger!);
    expect(screen.getByRole("dialog")).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});
