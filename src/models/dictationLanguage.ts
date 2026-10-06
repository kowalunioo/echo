import type { ModelLanguages, UiLanguage } from "../bindings";

/** The stored Dictation Language value that means "Automatic" (dictation-language.md rule 1). */
export const AUTOMATIC = "automatic";

/** The languages pinned under "Automatic", in this order (spec "UI"). */
const PINNED = ["pl", "en"];

/** The language part of a code without its regional variant; Norwegian "nb" reads as "no" (rule 5). */
function base(code: string): string {
  const primary = (code.split(/[-_]/)[0] ?? code).toLowerCase();
  return primary === "nb" ? "no" : primary;
}

/** The Model's own code for `intent`, ignoring regional variants, or `null` (rule 5). */
export function findLanguage(intent: string, languages: readonly string[]): string | null {
  if (languages.includes(intent)) return intent;
  const wanted = base(intent);
  return languages.find((code) => base(code) === wanted) ?? null;
}

/**
 * The effective language for the active Model: a language code or `AUTOMATIC` (rule 4). Mirrors
 * `resolve` in `src-tauri/src/dictation/language.rs`, which decides what the Engine receives.
 */
export function resolveLanguage(intent: string, model: ModelLanguages): string {
  const chosen = intent === AUTOMATIC ? null : findLanguage(intent, model.languages);
  if (chosen) return chosen;
  if (model.automatic) return AUTOMATIC;
  return findLanguage("en", model.languages) ?? model.languages[0] ?? AUTOMATIC;
}

/** Whisper's code for Javanese predates the ISO 639-1 one. */
const DISPLAY_CODE: Record<string, string> = { jw: "jv" };

/** A language's name in the UI Language, capitalised as a list entry. */
export function languageName(code: string, uiLanguage: UiLanguage): string {
  let name: string | undefined;
  try {
    name = new Intl.DisplayNames([uiLanguage], { type: "language" }).of(DISPLAY_CODE[code] ?? code);
  } catch {
    name = undefined;
  }
  const text = name ?? code;
  return text.charAt(0).toLocaleUpperCase(uiLanguage) + text.slice(1);
}

/**
 * The picker's values in order: `AUTOMATIC` if the Model detects, then Polish and English, then
 * the other languages by name in the UI Language (spec "UI").
 */
export function languageOptions(model: ModelLanguages, uiLanguage: UiLanguage): string[] {
  const pinned = PINNED.map((code) => findLanguage(code, model.languages)).filter(
    (code): code is string => code !== null,
  );
  const collator = new Intl.Collator(uiLanguage);
  const rest = model.languages
    .filter((code) => !pinned.includes(code))
    .map((code) => ({ code, name: languageName(code, uiLanguage) }))
    .sort((a, b) => collator.compare(a.name, b.name))
    .map(({ code }) => code);
  return [...(model.automatic ? [AUTOMATIC] : []), ...pinned, ...rest];
}

/** Text folded for search: lower case without diacritics, so "polsk" finds "Polski". */
export function searchKey(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}
