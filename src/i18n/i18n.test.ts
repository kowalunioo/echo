import { describe, expect, it } from "vitest";

import { en } from "./en";
import { changeUiLanguage, formatDate, formatNumber, uiLanguageForLocale } from "./index";
import { pl } from "./pl";

/** Every leaf key path of a nested string record, e.g. "nav.history". */
function keyPaths(value: unknown, prefix = ""): string[] {
  if (typeof value !== "object" || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) =>
    keyPaths(child, prefix ? `${prefix}.${key}` : key),
  );
}

function leaves(value: unknown): unknown[] {
  if (typeof value !== "object" || value === null) return [value];
  return Object.values(value).flatMap(leaves);
}

// settings-and-first-run.md acceptance test 8: translation completeness.
describe("translations", () => {
  it("Polish and English have exactly the same keys", () => {
    expect(keyPaths(pl).sort()).toEqual(keyPaths(en).sort());
  });

  it("every string is non-empty in both languages", () => {
    for (const value of [...leaves(en), ...leaves(pl)]) {
      expect(typeof value).toBe("string");
      expect((value as string).trim()).not.toBe("");
    }
  });
});

// settings-and-first-run.md rule 10.
describe("formatting", () => {
  it("formats numbers and dates per the UI Language", async () => {
    const date = new Date(2026, 9, 6, 14, 5);

    await changeUiLanguage("pl");
    expect(formatNumber(12345.5)).toBe("12 345,5");
    expect(formatDate(date, { dateStyle: "long" })).toBe("6 października 2026");

    await changeUiLanguage("en");
    expect(formatNumber(12345.5)).toBe("12,345.5");
    expect(formatDate(date, { dateStyle: "long" })).toBe("6 October 2026");
  });
});

// settings-and-first-run.md acceptance test 6 (frontend copy of the rule).
describe("uiLanguageForLocale", () => {
  it("chooses Polish for Polish Windows", () => {
    expect(uiLanguageForLocale("pl-PL")).toBe("pl");
    expect(uiLanguageForLocale("pl")).toBe("pl");
  });

  it("chooses English for anything else", () => {
    expect(uiLanguageForLocale("de-DE")).toBe("en");
    expect(uiLanguageForLocale("en-GB")).toBe("en");
    expect(uiLanguageForLocale("")).toBe("en");
    expect(uiLanguageForLocale(undefined)).toBe("en");
  });
});
