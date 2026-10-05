import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  applyTheme,
  bootstrapTheme,
  getSystemThemeDark,
  normalizeThemePreference,
  observeSystemTheme,
  resolveEffectiveTheme,
  SYSTEM_THEME_QUERY,
} from "./theme";

describe("theme model", () => {
  it.each([
    ["dark", "dark"],
    ["light", "light"],
    ["system", "system"],
  ])("accepts explicit preference %s", (value, expected) => {
    expect(normalizeThemePreference(value)).toBe(expected);
  });

  it.each([undefined, null, "", "sepia", 1, false])("falls back to dark for %s", (value) => {
    expect(normalizeThemePreference(value)).toBe("dark");
  });

  it("resolves explicit themes without consulting the OS", () => {
    expect(resolveEffectiveTheme("dark", false)).toBe("dark");
    expect(resolveEffectiveTheme("dark", true)).toBe("dark");
    expect(resolveEffectiveTheme("light", false)).toBe("light");
    expect(resolveEffectiveTheme("light", true)).toBe("light");
  });

  it("resolves System from the OS preference", () => {
    expect(resolveEffectiveTheme("system", true)).toBe("dark");
    expect(resolveEffectiveTheme("system", false)).toBe("light");
  });
});

describe("theme DOM controller", () => {
  beforeEach(() => {
    delete document.documentElement.dataset.theme;
    document.documentElement.style.colorScheme = "";
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("applies the root data attribute and native color scheme", () => {
    expect(applyTheme("light")).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.style.colorScheme).toBe("light");

    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
  });

  it("bootstraps an invalid value to dark", () => {
    expect(bootstrapTheme("unsupported")).toEqual({ preference: "dark", effectiveTheme: "dark" });
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});

describe("system theme observer", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    Reflect.deleteProperty(window, "matchMedia");
  });

  it("notifies on OS changes and unsubscribes cleanly", () => {
    let listener: ((event: MediaQueryListEvent) => void) | undefined;
    const media = {
      matches: false,
      addEventListener: vi.fn((_type: string, next: (event: MediaQueryListEvent) => void) => { listener = next; }),
      removeEventListener: vi.fn(),
    } as unknown as MediaQueryList;
    const matchMedia = vi.fn((query: string) => {
      expect(query).toBe(SYSTEM_THEME_QUERY);
      return media;
    });
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: matchMedia });

    const onChange = vi.fn();
    const unsubscribe = observeSystemTheme(onChange);
    listener?.({ matches: true } as MediaQueryListEvent);

    expect(onChange).toHaveBeenCalledWith(true);
    expect(media.addEventListener).toHaveBeenCalledOnce();
    unsubscribe();
    expect(media.removeEventListener).toHaveBeenCalledOnce();
  });

  it("uses a dark-safe fallback when matchMedia is unavailable", () => {
    expect(getSystemThemeDark()).toBe(false);
    expect(observeSystemTheme(vi.fn())).toEqual(expect.any(Function));
  });
});
