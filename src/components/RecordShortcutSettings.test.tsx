import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { changeUiLanguage } from "../i18n";
import { useSettings } from "../store/settings";
import { backend } from "../test/backend";
import { RecordShortcutSettings } from "./RecordShortcutSettings";

function key(key: string, pressed: boolean) {
  act(() => {
    backend.emit("captured-key-event", { key, pressed });
  });
}

function field() {
  return screen.getByTestId("record-shortcut-field");
}

async function renderPage() {
  await useSettings.getState().load();
  render(<RecordShortcutSettings />);
  return screen.findByRole("button", { name: /currently Ctrl \+ Space/ });
}

beforeEach(async () => {
  await changeUiLanguage("en");
});

describe("Record Shortcut settings", () => {
  it("shows the current shortcut as key caps and the default mode", async () => {
    const shortcutField = await renderPage();

    expect(shortcutField).toHaveTextContent("Ctrl+Space");
    expect(screen.getByRole("radio", { name: /Hold to record/ })).toBeChecked();
    expect(screen.getByRole("button", { name: "Reset to default" })).toBeDisabled();
  });

  it("shows the shortcut stored in the settings", async () => {
    backend.settings = { ...backend.settings, recordShortcut: "Ctrl+Win", shortcutMode: "toggle" };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);

    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Win/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Press to start/ })).toBeChecked();
  });

  // record-shortcut.md acceptance test 14: Ctrl down, Space down, Space up → "Ctrl+Space".
  it("captures a combination with a main key and saves it", async () => {
    await userEvent.click(await renderPage());
    expect(screen.getByText("Press the new shortcut…")).toBeInTheDocument();
    expect(backend.commandsCalled("begin_shortcut_capture")).toHaveLength(1);

    key("RightCtrl", true);
    key("LeftAlt", true);
    key("D", true);
    expect(field()).toHaveTextContent("Ctrl+Alt+D");
    key("D", false);

    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Alt \+ D/ }),
    ).toBeInTheDocument();
    expect(backend.commandsCalled("set_record_shortcut")[0]?.args).toEqual({
      combination: "Ctrl+Alt+D",
    });
    expect(backend.settings.recordShortcut).toBe("Ctrl+Alt+D");
  });

  // Acceptance test 14: Ctrl down, Win down, Win up, Ctrl up → "Ctrl+Win".
  it("captures a modifier-only combination when all modifiers are released", async () => {
    await userEvent.click(await renderPage());

    key("LeftCtrl", true);
    key("LeftWin", true);
    key("LeftWin", false);
    expect(backend.commandsCalled("set_record_shortcut")).toHaveLength(0);
    key("LeftCtrl", false);

    await waitFor(() => {
      expect(backend.settings.recordShortcut).toBe("Ctrl+Win");
    });
  });

  // Acceptance test 14: click outside → no change.
  it("ends capture without changes on a click outside the field", async () => {
    await userEvent.click(await renderPage());
    key("LeftCtrl", true);

    await userEvent.click(screen.getByText("Starts and stops a Recording from any application."));

    expect(backend.commandsCalled("end_shortcut_capture")).toHaveLength(1);
    expect(backend.commandsCalled("set_record_shortcut")).toHaveLength(0);
    expect(field()).toHaveTextContent("Ctrl+Space");
  });

  it("ends capture without changes when the window loses focus", async () => {
    await userEvent.click(await renderPage());

    act(() => {
      window.dispatchEvent(new Event("blur"));
    });

    expect(backend.commandsCalled("end_shortcut_capture")).toHaveLength(1);
    expect(screen.queryByText("Press the new shortcut…")).not.toBeInTheDocument();
  });

  it("ends capture without changes when Escape is pressed alone", async () => {
    await userEvent.click(await renderPage());

    key("Escape", true);

    expect(backend.commandsCalled("end_shortcut_capture")).toHaveLength(1);
    expect(backend.commandsCalled("set_record_shortcut")).toHaveLength(0);
  });

  it("explains a rejected proposal and keeps the previous shortcut", async () => {
    backend.rejectedShortcuts.set("Space", { kind: "notAllowed", problem: "needsModifier" });
    await userEvent.click(await renderPage());

    key("Space", true);
    key("Space", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Space alone would stop working for typing. Add Ctrl, Alt, Shift or Win. Your previous shortcut stays active.",
    );
    expect(field()).toHaveTextContent("Ctrl+Space");
    expect(backend.settings.recordShortcut).toBe("Ctrl+Space");
  });

  it("shows why an allowed shortcut could not be activated", async () => {
    backend.rejectedShortcuts.set("F9", {
      kind: "activationFailed",
      reason: "the keyboard hook is not running",
    });
    await userEvent.click(await renderPage());

    key("F9", true);
    key("F9", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Echo could not activate F9: the keyboard hook is not running",
    );
  });

  it("names keys in the UI Language", async () => {
    await renderPage();
    await act(() => changeUiLanguage("pl"));

    expect(
      await screen.findByRole("button", { name: "Zmień skrót nagrywania, obecnie Ctrl + Spacja" }),
    ).toBeInTheDocument();
  });

  // Rule 24 through the settings' reset.
  it("resets to the default", async () => {
    backend.settings = { ...backend.settings, recordShortcut: "F9" };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);
    const reset = await screen.findByRole("button", { name: "Reset to default" });
    expect(reset).toBeEnabled();

    await userEvent.click(reset);

    expect(backend.commandsCalled("reset_setting")[0]?.args).toEqual({ key: "recordShortcut" });
    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Space/ }),
    ).toBeInTheDocument();
  });

  it("switches the mode and saves it", async () => {
    await renderPage();

    await userEvent.click(screen.getByRole("radio", { name: /Press to start, press again/ }));

    expect(screen.getByRole("radio", { name: /Press to start, press again/ })).toBeChecked();
    await waitFor(() => {
      expect(backend.settings.shortcutMode).toBe("toggle");
    });
  });

  it("follows a shortcut changed outside the window", async () => {
    await renderPage();

    act(() => {
      backend.changeSettings({ recordShortcut: "RightAlt" });
    });

    expect(await screen.findByRole("button", { name: /currently Right Alt/ })).toBeInTheDocument();
  });
});
