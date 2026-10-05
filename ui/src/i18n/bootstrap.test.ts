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
      expect(document.documentElement.dataset.theme).toBe("dark");
    });
    await bootstrapPresentation(async () => ({ ui: { language: "zh-CN" } } as AppConfig), render, ["en-US"]);
    expect(render).toHaveBeenCalledOnce();
    expect(i18n.options.fallbackLng).toEqual(["en"]);
  });

  it.each(["light", "dark"] as const)("applies saved %s before rendering", async (theme) => {
    const render = vi.fn(() => expect(document.documentElement.dataset.theme).toBe(theme));
    await bootstrapPresentation(async () => ({ ui: { theme } } as AppConfig), render, ["en-US"]);
    expect(render).toHaveBeenCalledOnce();
  });

  it("applies System using the current OS preference before rendering", async () => {
    const originalMatchMedia = window.matchMedia;
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      writable: true,
      value: vi.fn().mockReturnValue({ matches: true }),
    });
    const render = vi.fn(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    await bootstrapPresentation(async () => ({ ui: { theme: "system" } } as AppConfig), render, ["en-US"]);
    expect(render).toHaveBeenCalledOnce();
    if (originalMatchMedia) Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: originalMatchMedia });
    else Reflect.deleteProperty(window, "matchMedia");
  });
  it("still renders on config failure using system language", async () => {
    const get = vi.fn().mockRejectedValue({ code: "CONFIG_INVALID" });
    const render = vi.fn(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    await bootstrapPresentation(get, render, ["zh-CN"]);
    expect(render).toHaveBeenCalledOnce();
    expect(i18n.language).toBe("zh-CN");
    await expect(get()).rejects.toEqual({ code: "CONFIG_INVALID" });
  });
  it("defaults deterministically to English without config or locale", async () => {
    await bootstrapPresentation(async () => null, () => {}, []);
    expect(document.documentElement.lang).toBe("en");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
