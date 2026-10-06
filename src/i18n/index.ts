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
  if (i18n.language !== language) await i18n.changeLanguage(language);
  document.documentElement.lang = language;
}

/** The locale used for dates and numbers in each UI Language (rule 10). */
const FORMAT_LOCALES: Partial<Record<string, string>> & Record<UiLanguage, string> = {
  pl: "pl-PL",
  en: "en-GB",
};

function formatLocale(): string {
  return FORMAT_LOCALES[i18n.language] ?? FORMAT_LOCALES.en;
}

/** A number formatted for the current UI Language, e.g. 12 345,5 or 12,345.5. */
export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(formatLocale(), options).format(value);
}

/** A date and/or time formatted for the current UI Language. */
export function formatDate(
  value: Date | number,
  options: Intl.DateTimeFormatOptions = { dateStyle: "medium", timeStyle: "short" },
): string {
  return new Intl.DateTimeFormat(formatLocale(), options).format(value);
}

export { i18n };
