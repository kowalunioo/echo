import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import type { ModelId } from "../bindings";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState({ ...initialShell, page: "vocabulary" }, true);
  await changeUiLanguage("en");
});

function activate(id: ModelId) {
  const entry = backend.models.models.find((m) => m.id === id);
  if (entry) entry.downloaded = true;
  backend.models.active = id;
  backend.models.activeState = "ready";
}

async function field() {
  render(<App />);
  return screen.findByRole("textbox", { name: "Add a word or phrase" });
}

function chips() {
  const list = screen.queryByRole("list", { name: "Vocabulary entries" });
  return list
    ? within(list)
        .getAllByRole("listitem")
        .map((li) => li.textContent)
    : [];
}

describe("Vocabulary page", () => {
  it("starts empty and has no placeholder", async () => {
    await field();
    expect(screen.getByText(/Your Vocabulary is empty/)).toBeVisible();
    expect(screen.queryByText("Coming soon")).not.toBeInTheDocument();
    expect(
      screen.getByText("Vocabulary fills 0 of the 672 characters Whisper Models read"),
    ).toBeVisible();
  });

  // Acceptance test 1 and rule 7: Enter adds the normalised entry, saved at once.
  it("adds a normalised entry with Enter and saves it", async () => {
    const input = await field();
    await userEvent.type(input, '  "Claude   Code" {Enter}');
    await waitFor(() => {
      expect(backend.settings.vocabulary).toEqual(["Claude Code"]);
    });
    expect(chips()).toEqual(["Claude Code"]);
    expect(input).toHaveValue("");
  });

  it("adds with the Add button, keeping list order and spelling", async () => {
    const input = await field();
    await userEvent.type(input, "GitHub");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    await userEvent.type(input, "tauri");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => {
      expect(backend.settings.vocabulary).toEqual(["GitHub", "tauri"]);
    });
  });

  // Acceptance test 2.
  it("rejects a duplicate with a message and leaves the list unchanged", async () => {
    backend.settings.vocabulary = ["GitHub"];
    const input = await field();
    await userEvent.type(input, "github{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "“GitHub” is already in your Vocabulary.",
    );
    expect(backend.settings.vocabulary).toEqual(["GitHub"]);
    expect(chips()).toEqual(["GitHub"]);

    await userEvent.type(input, "x");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  // Acceptance test 3.
  it("disables Add with a hint for more than 50 characters", async () => {
    const input = await field();
    await userEvent.type(input, "a".repeat(51));
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("An entry can have at most 50 characters.");

    await userEvent.type(input, "{Backspace}");
    expect(screen.getByRole("button", { name: "Add" })).toBeEnabled();
    await userEvent.type(input, "{Enter}");
    await waitFor(() => {
      expect(backend.settings.vocabulary).toEqual(["a".repeat(50)]);
    });
  });

  // Rule 6 and "UI": each chip has a remove control labelled "Remove <entry>".
  it("removes an entry with its chip's remove control", async () => {
    backend.settings.vocabulary = ["Echo", "GitHub", "Tauri"];
    await field();
    await userEvent.click(screen.getByRole("button", { name: "Remove GitHub" }));
    await waitFor(() => {
      expect(backend.settings.vocabulary).toEqual(["Echo", "Tauri"]);
    });
    expect(chips()).toEqual(["Echo", "Tauri"]);
  });

  it("offers Undo after a removal, restoring the entry in its place", async () => {
    backend.settings.vocabulary = ["Echo", "GitHub", "Tauri"];
    await field();
    await userEvent.click(screen.getByRole("button", { name: "Remove GitHub" }));
    expect(within(screen.getByRole("main")).getByRole("status")).toHaveTextContent(
      "“GitHub” removed",
    );

    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(backend.settings.vocabulary).toEqual(["Echo", "GitHub", "Tauri"]);
    });
    expect(chips()).toEqual(["Echo", "GitHub", "Tauri"]);
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("offers Undo for 5 seconds only", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      backend.settings.vocabulary = ["Echo"];
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      await field();
      await user.click(screen.getByRole("button", { name: "Remove Echo" }));
      act(() => {
        vi.advanceTimersByTime(4_900);
      });
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      act(() => {
        vi.advanceTimersByTime(200);
      });
      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps keyboard focus in the list after a removal", async () => {
    backend.settings.vocabulary = ["Echo", "GitHub", "Tauri"];
    await field();
    screen.getByRole("button", { name: "Remove GitHub" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Remove Tauri" })).toHaveFocus();
    });

    await userEvent.keyboard("{Enter}");
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Remove Echo" })).toHaveFocus();
    });

    await userEvent.keyboard("{Enter}");
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Add a word or phrase" })).toHaveFocus();
    });
  });

  // Acceptance test 11 in the UI.
  it("warns from 80% of the hint budget", async () => {
    // 12 entries of 43 characters joined with ", " make 538 characters.
    backend.settings.vocabulary = Array.from(
      { length: 12 },
      (_, i) => `${String(i).padStart(2, "0")}${"x".repeat(41)}`,
    );
    await field();
    expect(
      screen.getByText("Vocabulary fills 538 of the 672 characters Whisper Models read"),
    ).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Vocabulary is nearly full — Whisper Models ignore entries past the limit.",
    );
  });

  it("does not warn below 80%", async () => {
    // 6 entries of 48 characters joined with ", " make 298 characters → ceil(99.3) = 100 → 44%.
    backend.settings.vocabulary = Array.from(
      { length: 6 },
      (_, i) => `${String(i)}${"x".repeat(47)}`,
    );
    await field();
    expect(
      screen.getByText("Vocabulary fills 298 of the 672 characters Whisper Models read"),
    ).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("explains spelling corrections when the active Model takes no prompt", async () => {
    activate("parakeetTdt06bV3");
    await field();
    const note = await screen.findByRole("note");
    expect(note).toHaveTextContent(
      /Parakeet TDT 0\.6B v3 can't use your Vocabulary while it listens, so Echo fixes the spelling/,
    );
    // vocabulary.md rule 10.1: entries outside plain Latin letters and digits are not corrected.
    expect(note).toHaveTextContent(/entries with letters such as ą, ł or é are not/);
  });

  it("has no correction note for a Whisper Model", async () => {
    activate("whisperLargeV3Turbo");
    await field();
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });

  it("is labelled in Polish too", async () => {
    backend.settings.uiLanguage = "pl";
    backend.settings.vocabulary = ["GitHub"];
    render(<App />);
    const input = await screen.findByRole("textbox", { name: "Dodaj słowo lub frazę" });
    expect(screen.getByRole("button", { name: "Usuń GitHub" })).toBeVisible();
    expect(
      screen.getByText("Słownik zajmuje 6 z 672 znaków, które odczytują modele Whisper"),
    ).toBeVisible();
    await userEvent.type(input, "GITHUB{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("„GitHub” jest już w Słowniku.");
  });
});
