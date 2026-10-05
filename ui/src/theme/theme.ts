export type ThemePreference = "system" | "light" | "dark";
export type EffectiveTheme = "light" | "dark";

export const SYSTEM_THEME_QUERY = "(prefers-color-scheme: dark)";

let bootstrappedPreference: ThemePreference = "dark";

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === "system" || value === "light" || value === "dark";
}

/** Invalid or absent persisted values keep the M8 dark appearance. */
export function normalizeThemePreference(value: unknown): ThemePreference {
  return isThemePreference(value) ? value : "dark";
}

export const resolveThemePreference = normalizeThemePreference;

export function resolveEffectiveTheme(
  preference: ThemePreference | string | null | undefined,
  systemDark = false,
): EffectiveTheme {
  const normalized = normalizeThemePreference(preference);
  if (normalized === "system") return systemDark ? "dark" : "light";
  return normalized;
}

export function getSystemThemeDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return true;
  try {
    return window.matchMedia(SYSTEM_THEME_QUERY).matches;
  } catch {
    return true;
  }
}

export function applyTheme(theme: EffectiveTheme | string): EffectiveTheme {
  const effective: EffectiveTheme = theme === "light" ? "light" : "dark";
  if (typeof document !== "undefined") {
    const root = document.documentElement;
    root.dataset.theme = effective;
    root.style.colorScheme = effective;
  }
  return effective;
}

export type ThemeBootstrapResult = {
  preference: ThemePreference;
  effectiveTheme: EffectiveTheme;
};

/** Resolve and apply the startup preference before React's first render. */
export function bootstrapTheme(
  persistedPreference: unknown,
  systemDark = getSystemThemeDark(),
): ThemeBootstrapResult {
  const preference = normalizeThemePreference(persistedPreference);
  const effectiveTheme = resolveEffectiveTheme(preference, systemDark);
  bootstrappedPreference = preference;
  applyTheme(effectiveTheme);
  return { preference, effectiveTheme };
}

export function getBootstrappedThemePreference(): ThemePreference {
  return bootstrappedPreference;
}

/** Subscribe to OS theme changes. The caller owns the initial resolution. */
export function observeSystemTheme(onChange: (systemDark: boolean) => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return () => undefined;

  let media: MediaQueryList;
  try {
    media = window.matchMedia(SYSTEM_THEME_QUERY);
  } catch {
    return () => undefined;
  }

  const listener = (event: MediaQueryListEvent) => onChange(event.matches);
  if (typeof media.addEventListener === "function") media.addEventListener("change", listener);
  else media.addListener(listener);

  return () => {
    if (typeof media.removeEventListener === "function") media.removeEventListener("change", listener);
    else media.removeListener(listener);
  };
}
