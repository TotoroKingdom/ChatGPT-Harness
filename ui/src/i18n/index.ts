import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import type { AppConfig, UiLanguage } from "@protocol/v1";
import en from "./en.json";
import zhCN from "./zh-CN.json";
import { resolveLanguage, supportedLanguages, systemLocales } from "./resolve-language";

export const i18n = i18next.createInstance();
i18n.use(initReactI18next);
i18n.on("languageChanged", (language) => {
  if (typeof document !== "undefined") document.documentElement.lang = language;
});

export async function initializeLanguage(language: UiLanguage = "en") {
  if (i18n.isInitialized) await i18n.changeLanguage(language);
  else await i18n.init({
    lng: language,
    fallbackLng: "en",
    supportedLngs: [...supportedLanguages],
    load: "currentOnly",
    resources: { en: { translation: en }, "zh-CN": { translation: zhCN } },
    interpolation: { escapeValue: false },
    returnNull: false,
    react: { useSuspense: false },
  });
}

/** A failed bootstrap read does not replace or clear Provider's recovery read. */
export async function bootstrapPresentation(
  configGet: () => Promise<AppConfig | null>,
  render: () => void,
  locales: readonly string[] = systemLocales(),
) {
  let saved: UiLanguage | undefined;
  try { saved = (await configGet())?.ui.language; } catch { /* Provider owns recovery. */ }
  await initializeLanguage(resolveLanguage(saved, locales));
  render();
}
