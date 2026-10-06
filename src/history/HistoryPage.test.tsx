import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState({ ...initialShell, page: "history" }, true);
  await changeUiLanguage("en");
});

afterEach(() => {
  vi.useRealTimers();
});

async function openHistory(user = userEvent.setup()) {
  render(<App />);
  await screen.findByRole("heading", { level: 1, name: "History" });
  return user;
}

function list() {
  return screen.getByRole("list", { name: "Transcripts" });
}

function entryTexts() {
  return within(list())
    .getAllByRole("listitem")
    .map((item) => item.querySelector("[data-entry-text]")?.textContent);
}

function entry(text: string) {
  const item = within(list())
    .getAllByRole("listitem")
    .find((li) => li.querySelector("[data-entry-text]")?.textContent === text);
  if (!item) throw new Error(`no entry "${text}"`);
  return within(item);
}

describe("History page", () => {
  it("shows the empty state with the Record Shortcut", async () => {
    await openHistory();

    expect(screen.getByText(/No Transcripts yet/)).toHaveTextContent(
      "No Transcripts yet. Press Ctrl + Space and speak.",
    );
    expect(screen.queryByRole("button", { name: "Clear all" })).not.toBeInTheDocument();
  });

  it("shows the Record Shortcut actually set, in the UI Language", async () => {
    backend.settings.recordShortcut = "RightAlt";
    await openHistory();

    expect(screen.getByText(/No Transcripts yet/)).toHaveTextContent(
      "No Transcripts yet. Press Right Alt and speak.",
    );
  });

  it("lists entries newest first with their local date and time", async () => {
    backend.addHistoryEntry("older", new Date(2026, 9, 5, 14, 3).getTime());
    backend.addHistoryEntry("newest", new Date(2026, 9, 6, 9, 30).getTime());
    await openHistory();

    expect(entryTexts()).toEqual(["newest", "older"]);
    expect(entry("older").getByText("5 October 2026, 14:03")).toBeInTheDocument();
  });

  it("formats dates in Polish when the UI Language is Polish", async () => {
    backend.settings.uiLanguage = "pl";
    backend.addHistoryEntry("tekst", new Date(2026, 9, 5, 14, 3).getTime());
    render(<App />);

    expect(await screen.findByText("5 października 2026, 14:03")).toBeInTheDocument();
  });

  // history.md acceptance test 11.
  it("shows a new Dictation's entry without reloading", async () => {
    backend.addHistoryEntry("first");
    await openHistory();

    act(() => {
      backend.addHistoryEntry("from a Dictation");
    });

    expect(entryTexts()).toEqual(["from a Dictation", "first"]);
  });

  // history.md acceptance test 8.
  it("copies the exact text and confirms it", async () => {
    backend.addHistoryEntry("Zażółć gęślą jaźń\nline two");
    const user = await openHistory();

    await user.click(entry("Zażółć gęślą jaźń\nline two").getByRole("button", { name: "Copy" }));

    expect(await navigator.clipboard.readText()).toBe("Zażółć gęślą jaźń\nline two");
    expect(screen.getByRole("status")).toHaveTextContent("Copied");
  });

  // history.md acceptance test 9 (frontend part).
  it("deletes at once and Undo restores the entry with its original time", async () => {
    const original = backend.addHistoryEntry("keep me", new Date(2026, 0, 2, 3, 4).getTime());
    backend.addHistoryEntry("newer");
    const user = await openHistory();

    await user.click(entry("keep me").getByRole("button", { name: "Delete" }));

    expect(entryTexts()).toEqual(["newer"]);
    expect(backend.history.map((e) => e.text)).toEqual(["newer"]);
    expect(screen.getByRole("status")).toHaveTextContent("Transcript deleted");

    await user.click(screen.getByRole("button", { name: "Undo" }));

    expect(entryTexts()).toEqual(["newer", "keep me"]);
    expect(backend.history.find((e) => e.text === "keep me")).toEqual(original);
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("offers Undo for 5 seconds only", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    backend.addHistoryEntry("gone");
    const user = await openHistory(userEvent.setup({ advanceTimers: vi.advanceTimersByTime }));

    await user.click(entry("gone").getByRole("button", { name: "Delete" }));
    act(() => {
      vi.advanceTimersByTime(4_900);
    });
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    expect(backend.history).toEqual([]);
  });

  // history.md acceptance test 10 (with the fake backend standing in for Insertion).
  it("re-inserts the entry's text", async () => {
    backend.addHistoryEntry("Ala ma kota");
    const user = await openHistory();

    await user.click(entry("Ala ma kota").getByRole("button", { name: "Re-insert" }));

    expect(backend.reinserted).toEqual(["Ala ma kota"]);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says so when Re-insert fails", async () => {
    backend.addHistoryEntry("Ala ma kota");
    backend.failing.set("reinsert_history_entry", "insertion failed: blocked");
    const user = await openHistory();

    await user.click(entry("Ala ma kota").getByRole("button", { name: "Re-insert" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't insert the text.");
  });

  it("clears everything only after confirmation", async () => {
    backend.addHistoryEntry("one");
    backend.addHistoryEntry("two");
    const user = await openHistory();

    await user.click(screen.getByRole("button", { name: "Clear all" }));
    const dialog = screen.getByRole("alertdialog", { name: "Delete all Transcripts?" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(entryTexts()).toEqual(["two", "one"]);

    await user.click(screen.getByRole("button", { name: "Clear all" }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Delete all" }),
    );

    expect(backend.history).toEqual([]);
    expect(await screen.findByText(/No Transcripts yet/)).toBeInTheDocument();
  });

  it("collapses very long texts", async () => {
    const long = "word ".repeat(200).trim();
    backend.addHistoryEntry(long);
    const user = await openHistory();

    await user.click(screen.getByRole("button", { name: "Show more" }));

    expect(screen.getByRole("button", { name: "Show less" })).toBeInTheDocument();
    expect(entry(long).getByRole("button", { name: "Show less" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });
});

describe("History limit control", () => {
  it("shows the limit and what it means", async () => {
    await openHistory();

    expect(screen.getByRole("spinbutton", { name: "History limit" })).toHaveValue(5);
    expect(screen.getByText("Keeps the last 5 Transcripts")).toBeInTheDocument();
  });

  it("changes the limit with the steppers and saves it", async () => {
    const user = await openHistory();

    await user.click(screen.getByRole("button", { name: "Keep more Transcripts" }));
    await user.click(screen.getByRole("button", { name: "Keep more Transcripts" }));
    await user.click(screen.getByRole("button", { name: "Keep fewer Transcripts" }));

    expect(backend.settings.historyLimit).toBe(6);
    expect(screen.getByRole("spinbutton", { name: "History limit" })).toHaveValue(6);
  });

  // history.md acceptance test 5 (frontend part).
  it("lowering the limit leaves only the newest entries", async () => {
    for (const text of ["1", "2", "3", "4", "5"]) backend.addHistoryEntry(text);
    const user = await openHistory();

    const field = screen.getByRole("spinbutton", { name: "History limit" });
    await user.clear(field);
    await user.type(field, "2");
    // Typing alone deletes nothing: the value applies on Enter or when the field loses focus.
    expect(backend.history).toHaveLength(5);
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(entryTexts()).toEqual(["5", "4"]);
    });
    expect(backend.settings.historyLimit).toBe(2);
    expect(screen.getByText("Keeps the last 2 Transcripts")).toBeInTheDocument();
  });

  // history.md acceptance test 7 (UI part).
  it("does not save values outside 0–100 and stops the steppers at the ends", async () => {
    const user = await openHistory();
    const field = screen.getByRole("spinbutton", { name: "History limit" });

    await user.clear(field);
    await user.type(field, "101");
    await user.tab();

    expect(backend.settings.historyLimit).toBe(5);
    expect(field).toHaveValue(5);

    await user.clear(field);
    await user.type(field, "100{Enter}");
    expect(backend.settings.historyLimit).toBe(100);
    expect(screen.getByRole("button", { name: "Keep more Transcripts" })).toBeDisabled();

    await user.clear(field);
    await user.type(field, "0{Enter}");
    expect(backend.settings.historyLimit).toBe(0);
    expect(screen.getByRole("button", { name: "Keep fewer Transcripts" })).toBeDisabled();
  });

  // history.md rule 9.
  it("explains that History is off at limit 0", async () => {
    backend.settings.historyLimit = 0;
    await openHistory();

    expect(screen.getByText("Keeps nothing: new Transcripts are not saved")).toBeInTheDocument();
    expect(
      screen.getByText("History is off. Raise the limit to keep your Transcripts."),
    ).toBeInTheDocument();
  });

  it("uses Polish plural forms", async () => {
    backend.settings.uiLanguage = "pl";
    render(<App />);

    expect(await screen.findByText("Przechowuje ostatnie 5 transkrypcji")).toBeInTheDocument();

    act(() => {
      backend.changeSettings({ historyLimit: 2 });
    });
    expect(screen.getByText("Przechowuje ostatnie 2 transkrypcje")).toBeInTheDocument();

    act(() => {
      backend.changeSettings({ historyLimit: 1 });
    });
    expect(screen.getByText("Przechowuje ostatnią transkrypcję")).toBeInTheDocument();
  });
});
