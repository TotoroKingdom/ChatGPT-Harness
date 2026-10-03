import { describe, expect, it, vi } from "vitest";
import { bootstrapPresentation, i18n } from "./index";
import { resolveLanguage } from "./resolve-language";
import type { AppConfig } from "@protocol/v1";

describe("locale resolution", () => {
  it.each(["en", "en-US", "en-GB", "fr-FR", "de", "ja", "zh-TW", "zh-HK", "zh-MO", "zh-Hant", "zh-Hant-CN", "invalid!", ""])("falls back to English for %s", (locale) => {
    expect(resolveLanguage(undefined, [locale])).toBe("en");
  });
  it.each(["zh", "zh-CN", "zh-SG", "zh-Hans", "zh-Hans-CN"])("supports simplified locale %s", (locale) => {
    expect(resolveLanguage(undefined, [locale])).toBe("zh-CN");
  });
  it("uses preference before system and uses primary system locale", () => {
    expect(resolveLanguage("en", ["zh-CN"])).toBe("en");
    expect(resolveLanguage("zh-CN", ["en-US"])).toBe("zh-CN");
    expect(resolveLanguage(undefined, [])).toBe("en");
    expect(resolveLanguage(undefined, ["zh-TW", "zh-CN"])).toBe("en");
  });
});

describe("pre-render presentation bootstrap", () => {
  it("initializes saved locale and html lang before rendering", async () => {
    const render = vi.fn(() => {
      expect(i18n.language).toBe("zh-CN");
      expect(document.documentElement.lang).toBe("zh-CN");
    });
    await bootstrapPresentation(async () => ({ ui: { language: "zh-CN" } } as AppConfig), render, ["en-US"]);
    expect(render).toHaveBeenCalledOnce();
    expect(i18n.options.fallbackLng).toEqual(["en"]);
  });
  it("still renders on config failure using system language", async () => {
    const get = vi.fn().mockRejectedValue({ code: "CONFIG_INVALID" });
    const render = vi.fn();
    await bootstrapPresentation(get, render, ["zh-CN"]);
    expect(render).toHaveBeenCalledOnce();
    expect(i18n.language).toBe("zh-CN");
    await expect(get()).rejects.toEqual({ code: "CONFIG_INVALID" });
  });
  it("defaults deterministically to English without config or locale", async () => {
    await bootstrapPresentation(async () => null, () => {}, []);
    expect(document.documentElement.lang).toBe("en");
  });
});
