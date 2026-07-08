import { describe, it, expect } from "vitest";
import { saveRequestBody, normalizeConfig, AppConfig } from "./useConfigForm";

describe("saveRequestBody", () => {
  const base: AppConfig = {
    keyName: "AltRight",
    linuxEvdevCode: 100,
    windowsVk: null,
    language: "zh",
    engine: "local",
    model: "ggml-base",
    apiBaseUrl: "",
    polishLevel: "none",
    polishModel: "",
    polishApiBaseUrl: "",
    guiLanguage: "zh",
    transcriberApiKey: "",
    polisherApiKey: "",
    hasTranscriberApiKey: false,
    hasPolisherApiKey: false,
  };

  it("includes apiKey when transcriberApiKey is non-empty", () => {
    const result = saveRequestBody({ ...base, transcriberApiKey: "sk-abc" });
    expect(result).toHaveProperty("apiKey", "sk-abc");
  });

  it("excludes apiKey when transcriberApiKey is empty", () => {
    const result = saveRequestBody({ ...base, transcriberApiKey: "" });
    expect(result).not.toHaveProperty("apiKey");
  });

  it("includes polishApiKey when polisherApiKey is non-empty", () => {
    const result = saveRequestBody({ ...base, polisherApiKey: "sk-def" });
    expect(result).toHaveProperty("polishApiKey", "sk-def");
  });

  it("excludes polishApiKey when polisherApiKey is empty", () => {
    const result = saveRequestBody({ ...base, polisherApiKey: "" });
    expect(result).not.toHaveProperty("polishApiKey");
  });
});

describe("normalizeConfig", () => {
  it("sets undefined evdev/windows fields to null", () => {
    const input = {
      keyName: "AltRight",
      linuxEvdevCode: undefined,
      windowsVk: undefined,
      language: "zh",
      engine: "local",
      model: "ggml-base",
      apiBaseUrl: "",
      polishLevel: "none",
      polishModel: "",
      polishApiBaseUrl: "",
      guiLanguage: "zh",
      transcriberApiKey: "",
      polisherApiKey: "",
      hasTranscriberApiKey: false,
      hasPolisherApiKey: false,
    } as unknown as AppConfig;
    const result = normalizeConfig(input);
    expect(result.linuxEvdevCode).toBeNull();
    expect(result.windowsVk).toBeNull();
  });

  it("clears API keys in normalized output", () => {
    const input: AppConfig = {
      keyName: "AltRight",
      linuxEvdevCode: 100,
      windowsVk: null,
      language: "zh",
      engine: "local",
      model: "ggml-base",
      apiBaseUrl: "",
      polishLevel: "none",
      polishModel: "",
      polishApiBaseUrl: "",
      guiLanguage: "zh",
      transcriberApiKey: "sk-secret",
      polisherApiKey: "sk-other",
      hasTranscriberApiKey: true,
      hasPolisherApiKey: true,
    };
    const result = normalizeConfig(input);
    expect(result.transcriberApiKey).toBe("");
    expect(result.polisherApiKey).toBe("");
    expect(result.hasTranscriberApiKey).toBe(true);
    expect(result.hasPolisherApiKey).toBe(true);
  });
});
