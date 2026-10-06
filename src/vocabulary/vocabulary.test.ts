import { describe, expect, it } from "vitest";

import { acceptsPrompt, budgetPercent, checkEntry, normaliseEntry } from "./vocabulary";

/** Entries whose hint (joined with ", ") is exactly `chars` characters long. */
function entriesOfHintLength(chars: number): string[] {
  const entries: string[] = [];
  let length = 0;
  while (length < chars) {
    const separator = entries.length === 0 ? 0 : 2;
    const room = chars - length - separator;
    const size = Math.min(40, room);
    if (size <= 0) break;
    entries.push(`${String(entries.length)}${"x".repeat(size - String(entries.length).length)}`);
    length += separator + size;
  }
  return entries;
}

describe("Vocabulary entries", () => {
  // vocabulary.md acceptance test 1.
  it("normalises whitespace and removes <, > and quotes", () => {
    expect(normaliseEntry('  "Claude   Code" ')).toBe("Claude Code");
    expect(normaliseEntry("<Tauri>\t 2")).toBe("Tauri 2");
    expect(checkEntry('  "Claude   Code" ', [])).toEqual({ kind: "ok", entry: "Claude Code" });
  });

  // Acceptance test 2.
  it("rejects duplicates ignoring case", () => {
    expect(checkEntry("github", ["GitHub"])).toEqual({ kind: "duplicate", existing: "GitHub" });
    expect(checkEntry(" GITHUB ", ["Echo", "GitHub"]).kind).toBe("duplicate");
  });

  // Acceptance test 3.
  it("accepts 50 characters and rejects 51", () => {
    expect(checkEntry("a".repeat(50), []).kind).toBe("ok");
    expect(checkEntry("a".repeat(51), []).kind).toBe("tooLong");
    // Characters, not UTF-16 units.
    expect(checkEntry("ł".repeat(50), []).kind).toBe("ok");
    expect(checkEntry("😀".repeat(50), []).kind).toBe("ok");
  });

  it("treats blank input as nothing to add", () => {
    expect(checkEntry("   ", []).kind).toBe("empty");
    expect(checkEntry('""', []).kind).toBe("empty");
  });
});

describe("Hint budget", () => {
  // Acceptance test 11: ceil(chars / 3) · 100 / 224, floored.
  it("estimates one token per three characters of the joined hint", () => {
    const at538 = entriesOfHintLength(538);
    expect(at538.join(", ")).toHaveLength(538);
    expect(budgetPercent(at538)).toBe(80);

    const at300 = entriesOfHintLength(300);
    expect(at300.join(", ")).toHaveLength(300);
    expect(budgetPercent(at300)).toBe(44);
  });

  it("is 0% when empty and never more than 100%", () => {
    expect(budgetPercent([])).toBe(0);
    expect(budgetPercent(entriesOfHintLength(2000))).toBe(100);
  });
});

describe("Models and the hint", () => {
  it("only Parakeet takes no prompt", () => {
    expect(acceptsPrompt("whisperLargeV3Turbo")).toBe(true);
    expect(acceptsPrompt("whisperSmall")).toBe(true);
    expect(acceptsPrompt("parakeetTdt06bV3")).toBe(false);
  });
});
