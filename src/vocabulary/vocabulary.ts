import type { ModelId } from "../bindings";

/** The longest entry, in characters, after normalisation (vocabulary.md rule 3). */
export const MAX_ENTRY_CHARS = 50;

/** How many prompt tokens a Whisper Model reads (rule 9). */
export const HINT_TOKEN_BUDGET = 224;

/** From this share of the hint budget the indicator warns (vocabulary.md "UI"). */
export const WARNING_PERCENT = 80;

/** Entries are joined in list order with this into the hint (rule 8). */
export const HINT_SEPARATOR = ", ";

/** Models that take no text prompt; their Transcripts get spelling corrections (rule 10). */
const NO_PROMPT_MODELS: readonly ModelId[] = ["parakeetTdt06bV3"];

/**
 * The number of characters (Unicode code points, as the backend counts them), so "ł" and emoji
 * count once: the second half of a surrogate pair is not counted.
 */
export function charCount(text: string): number {
  let count = 0;
  for (let i = 0; i < text.length; i++) {
    const unit = text.charCodeAt(i);
    if (unit < 0xdc00 || unit > 0xdfff) count++;
  }
  return count;
}

/** Trims, collapses internal whitespace to single spaces and removes `<`, `>` and `"` (rule 2). */
export function normaliseEntry(input: string): string {
  return input
    .replace(/[<>"]/g, "")
    .split(/\s+/)
    .filter((word) => word !== "")
    .join(" ");
}

export type EntryCheck =
  | { kind: "ok"; entry: string }
  | { kind: "empty" }
  | { kind: "tooLong" }
  | { kind: "duplicate"; existing: string };

/** Whether `input` can be added to `entries`, and as what (rules 2–4). */
export function checkEntry(input: string, entries: readonly string[]): EntryCheck {
  const entry = normaliseEntry(input);
  if (entry === "") return { kind: "empty" };
  if (charCount(entry) > MAX_ENTRY_CHARS) return { kind: "tooLong" };
  const key = entry.toLowerCase();
  const existing = entries.find((e) => e.toLowerCase() === key);
  if (existing !== undefined) return { kind: "duplicate", existing };
  return { kind: "ok", entry };
}

/** Characters per prompt token in the pessimistic estimate (rule 9). */
const CHARS_PER_TOKEN = 3;

/** The hint budget in characters of the joined hint, as the user sees it (672). */
export const HINT_CHAR_BUDGET = HINT_TOKEN_BUDGET * CHARS_PER_TOKEN;

/** The characters of the joined hint the entries make (rule 8). */
export function hintChars(entries: readonly string[]): number {
  return charCount(entries.join(HINT_SEPARATOR));
}

/**
 * The share of the hint budget the entries use, estimated pessimistically as one token per 3
 * characters of the joined hint, rounded up (rule 9); floored and capped at 100%.
 */
export function budgetPercent(entries: readonly string[]): number {
  const tokens = Math.ceil(hintChars(entries) / CHARS_PER_TOKEN);
  return Math.min(100, Math.floor((tokens * 100) / HINT_TOKEN_BUDGET));
}

/** Whether the Model takes the Vocabulary as a prompt (rule 8) rather than as corrections. */
export function acceptsPrompt(model: ModelId): boolean {
  return !NO_PROMPT_MODELS.includes(model);
}
