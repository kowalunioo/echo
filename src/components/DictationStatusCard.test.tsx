import { act, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState(initialShell, true);
  await changeUiLanguage("en");
});

afterEach(() => {
  vi.useRealTimers();
});

async function card() {
  render(<App />);
  const status = await screen.findByRole("region", { name: "Status" });
  return within(status);
}

// PRODUCT.md principle 4 (honest state): the main window says what the Dictation is doing,
// whether or not the Overlay is shown.
describe("Dictation state in the sidebar", () => {
  it("says it is idle and how to start, with the Record Shortcut as key caps", async () => {
    const status = await card();

    expect(status.getByRole("status")).toHaveTextContent("Idle");
    const shortcut = status.getByTestId("status-shortcut");
    expect(shortcut).toHaveTextContent("Hold");
    expect(
      within(shortcut)
        .getAllByText(/Ctrl|Space/)
        .map((k) => k.tagName),
    ).toEqual(["KBD", "KBD"]);
    expect(shortcut).toHaveAccessibleName("Record Shortcut: hold Ctrl + Space");
    // The verb is the instruction; the mode's name is one hover away (CONTEXT.md).
    expect(shortcut).toHaveAttribute("title", "Push-to-Talk Mode");
  });

  it("says Press in Toggle Mode", async () => {
    backend.settings.shortcutMode = "toggle";
    const status = await card();

    expect(status.getByTestId("status-shortcut")).toHaveTextContent("Press");
    expect(status.getByTestId("status-shortcut")).toHaveAttribute("title", "Toggle Mode");
  });

  it("follows the Recording, with a timer while listening", async () => {
    const status = await card();
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });

    act(() => {
      backend.changeDictation({ state: "recording", listening: false });
    });
    expect(status.getByRole("status")).toHaveTextContent("Getting ready…");

    act(() => {
      backend.changeDictation({ state: "recording", listening: true });
    });
    expect(status.getByRole("status")).toHaveTextContent("Listening");
    expect(status.getByTestId("status-timer")).toHaveTextContent("0:00");

    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(status.getByTestId("status-timer")).toHaveTextContent("0:03");

    act(() => {
      backend.changeDictation({ state: "transcribing", listening: false });
    });
    expect(status.getByRole("status")).toHaveTextContent("Transcribing…");
    expect(status.queryByTestId("status-timer")).not.toBeInTheDocument();

    act(() => {
      backend.changeDictation({ state: "inserting" });
    });
    expect(status.getByRole("status")).toHaveTextContent("Inserting the text…");
  });

  it("shows the state even when the Overlay is turned off", async () => {
    backend.settings.showOverlay = false;
    const status = await card();

    act(() => {
      backend.changeDictation({ state: "recording", listening: true });
    });
    expect(status.getByRole("status")).toHaveTextContent("Listening");
  });

  it("says the last Dictation failed while its error is listed", async () => {
    backend.dictation = {
      ...backend.dictation,
      notices: [{ id: 1, kind: "transcriptionFailed", detail: "" }],
    };
    const status = await card();

    expect(await status.findByText("Last Dictation failed")).toBeVisible();
    expect(status.getByRole("status")).toHaveTextContent("Last Dictation failed");
  });

  it("is translated into Polish", async () => {
    backend.settings.uiLanguage = "pl";
    render(<App />);
    const status = within(await screen.findByRole("region", { name: "Stan" }));

    expect(status.getByRole("status")).toHaveTextContent("Czekam");
    expect(status.getByTestId("status-shortcut")).toHaveAccessibleName(
      "Skrót nagrywania: przytrzymaj Ctrl + Spacja",
    );
  });
});
