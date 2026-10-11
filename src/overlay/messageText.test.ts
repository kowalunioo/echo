import { afterEach, describe, expect, it } from "vitest";

import type { OverlayMessage, UiLanguage } from "../bindings";
import { changeUiLanguage, i18n } from "../i18n";
import { charCount } from "../vocabulary/vocabulary";
import { MAX_MESSAGE_CHARS, overlayMessageText } from "./messageText";

afterEach(async () => {
  await changeUiLanguage("en");
});

async function text(language: UiLanguage, message: OverlayMessage) {
  await changeUiLanguage(language);
  return overlayMessageText(message, i18n.t);
}

describe("Vocabulary Shortcut messages (vocabulary-shortcut.md rule 13)", () => {
  it("names the entry in both UI Languages", async () => {
    const added: OverlayMessage = { kind: "vocabularyAdded", entry: "GitHub" };
    expect(await text("en", added)).toBe('Added "GitHub" to Vocabulary');
    expect(await text("pl", added)).toBe("Dodano „GitHub” do Słownika");
    const present: OverlayMessage = { kind: "vocabularyAlreadyPresent", entry: "GitHub" };
    expect(await text("en", present)).toBe('"GitHub" is already in your Vocabulary');
    expect(await text("pl", present)).toBe("„GitHub” jest już w Słowniku");
  });

  it("tells what to select and names the shortcut", async () => {
    expect(await text("en", { kind: "vocabularyTooLong" })).toBe("Select one word or a short name");
    expect(await text("pl", { kind: "vocabularyTooLong" })).toBe(
      "Zaznacz jedno słowo lub krótką nazwę",
    );
    const nothing: OverlayMessage = { kind: "vocabularyNothingSelected", shortcut: "Ctrl+Alt+D" };
    expect(await text("en", nothing)).toBe("Select a word first, then press Ctrl+Alt+D");
    expect(await text("pl", nothing)).toBe("Najpierw zaznacz słowo, potem naciśnij Ctrl+Alt+D");
  });

  it("names the shortcut's keys in the UI Language, still on one line", async () => {
    const space: OverlayMessage = { kind: "vocabularyNothingSelected", shortcut: "Ctrl+Space" };
    expect(await text("pl", space)).toBe("Najpierw zaznacz słowo, potem naciśnij Ctrl+Spacja");
    const longest: OverlayMessage = {
      kind: "vocabularyNothingSelected",
      shortcut: "Ctrl+Alt+Shift+Win+D",
    };
    for (const language of ["en", "pl"] as const) {
      expect(charCount(await text(language, longest))).toBeLessThanOrEqual(MAX_MESSAGE_CHARS);
    }
  });

  // Acceptance test 8 (rule 14).
  it("shortens a long entry with … so the message fits in 60 characters", async () => {
    const entry = `${"Ł".repeat(24)} ${"b".repeat(25)}`;
    expect(charCount(entry)).toBe(50);
    for (const language of ["en", "pl"] as const) {
      for (const kind of ["vocabularyAdded", "vocabularyAlreadyPresent"] as const) {
        const message = await text(language, { kind, entry });
        expect(charCount(message), `${language} ${kind}: ${message}`).toBe(MAX_MESSAGE_CHARS);
        expect(message).toContain("Ł".repeat(10));
        expect(message).toMatch(/…["”]/);
      }
    }
  });
});
