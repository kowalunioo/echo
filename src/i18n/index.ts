import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import type { UiLanguage } from "../bindings";
import { en } from "./en";
import { pl } from "./pl";

export const resources = {
  en: { translation: en },
  pl: { translation: pl },
} as const;

export const UI_LANGUAGES: readonly UiLanguage[] = ["pl", "en"];

/**
 * The UI Language for a BCP 47 tag such as "pl-PL": Polish for any Polish variant, otherwise
 * English (settings-and-first-run.md rule 7). The backend applies the same rule to the Windows
 * display language; this copy covers the first paint before the backend answers.
 */
export function uiLanguageForLocale(locale: string | null | undefined): UiLanguage {
  const primary = locale?.split(/[-_]/)[0]?.toLowerCase();
  return primary === "pl" ? "pl" : "en";
}

export function initI18n(language: UiLanguage) {
  void i18n.use(initReactI18next).init({
    resources,
    lng: language,
    fallbackLng: "en",
    interpolation: { escapeValue: false }, // React already escapes
    returnNull: false,
  });
  document.documentElement.lang = language;
  return i18n;
}

export async function changeUiLanguage(language: UiLanguage) {
  await i18n.changeLanguage(language);
  document.documentElement.lang = language;
}

export { i18n };
