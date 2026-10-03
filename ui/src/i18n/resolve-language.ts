import type { UiLanguage } from "@protocol/v1";

export const supportedLanguages = ["en", "zh-CN"] as const;

export function systemLocales(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  return navigator.languages?.length ? navigator.languages : [navigator.language];
}

/** The first WebView locale is the system's preferred language, not a search
 * for any supported secondary language (Traditional Chinese stays unsupported). */
export function resolveLanguage(saved?: UiLanguage | null, locales: readonly string[] = []): UiLanguage {
  if (saved) return saved;
  const locale = locales[0]?.trim().toLowerCase();
  if (!locale || !/^[a-z]{2,3}(?:-[a-z0-9]{2,8})*$/.test(locale)) return "en";
  if (locale === "en" || locale.startsWith("en-")) return "en";
  if (/(?:^|-)hant(?:-|$)/.test(locale) || /^zh-(?:tw|hk|mo)(?:-|$)/.test(locale)) return "en";
  if (locale === "zh" || /^zh-(?:cn|sg|hans)(?:-|$)/.test(locale)) return "zh-CN";
  return "en";
}
