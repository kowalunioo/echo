import type { TFunction } from "i18next";

import type { OverlayMessage } from "../bindings";
import { keysOf } from "../shortcut/capture";
import { charCount } from "../vocabulary/vocabulary";

/** The longest Vocabulary Shortcut message, so it fits on one line (vocabulary-shortcut.md rule 14). */
export const MAX_MESSAGE_CHARS = 60;

const ELLIPSIS = "…";

/** The first `count` characters (code points) of `text`. */
function firstChars(text: string, count: number): string {
  return Array.from(text).slice(0, Math.max(0, count)).join("");
}

/**
 * A message naming a Vocabulary entry, with the entry shortened with "…" when the whole message
 * would be longer than {@link MAX_MESSAGE_CHARS} (vocabulary-shortcut.md rule 14).
 */
export function withEntry(render: (entry: string) => string, entry: string): string {
  const full = render(entry);
  if (charCount(full) <= MAX_MESSAGE_CHARS) return full;
  const room = MAX_MESSAGE_CHARS - charCount(render("")) - charCount(ELLIPSIS);
  return render(firstChars(entry, room) + ELLIPSIS);
}

/** The text of an Overlay message in the current UI Language (overlay.md rule 5). */
export function overlayMessageText(message: OverlayMessage, t: TFunction): string {
  switch (message.kind) {
    case "problem":
      return t(`overlay.messages.${message.problem}`);
    case "microphoneFallback":
      return t("overlay.messages.microphoneFallback");
    case "vocabularyAdded":
      return withEntry((entry) => t("overlay.messages.vocabularyAdded", { entry }), message.entry);
    case "vocabularyAlreadyPresent":
      return withEntry(
        (entry) => t("overlay.messages.vocabularyAlreadyPresent", { entry }),
        message.entry,
      );
    case "vocabularyTooLong":
      return t("overlay.messages.vocabularyTooLong");
    case "vocabularyReleaseKeys":
      return t("overlay.messages.vocabularyReleaseKeys");
    case "vocabularyNothingSelected":
      return t("overlay.messages.vocabularyNothingSelected", {
        shortcut: compactKeyLabel(message.shortcut, t),
      });
  }
}

/**
 * A combination with its keys named in the UI Language ("Ctrl+Spacja"), joined without spaces so
 * even the longest combination keeps the message on one line.
 */
function compactKeyLabel(combination: string, t: TFunction): string {
  const names = t("keys", { returnObjects: true }) as Partial<Record<string, string>>;
  return keysOf(combination)
    .map((key) => names[key] ?? key)
    .join("+");
}
