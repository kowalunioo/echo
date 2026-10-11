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

const SHORTCUT_NAME = {
  record: "Record Shortcut",
  cancel: "Cancel Shortcut",
  vocabulary: "Add selected text to Vocabulary",
} as const;

/** The reset icon next to a shortcut, described by the shortcut's name. */
function resetButton(target: keyof typeof SHORTCUT_NAME) {
  return screen.findByRole("button", {
    name: "Reset to default",
    description: SHORTCUT_NAME[target],
  });
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
  it("groups the shortcuts under a Shortcuts heading", async () => {
    await renderPage();
    expect(screen.getByRole("region", { name: "Shortcuts" })).toBeInTheDocument();
  });

  it("shows the current shortcut as key caps and the default mode", async () => {
    const shortcutField = await renderPage();

    expect(shortcutField).toHaveTextContent("Ctrl+Space");
    // record-shortcut.md "UI": each option is named by its mode, with the plain sentence as hint.
    const pushToTalk = screen.getByRole("radio", { name: "Push-to-Talk Mode" });
    expect(pushToTalk).toBeChecked();
    expect(pushToTalk).toHaveAccessibleDescription("Hold to record, let go to stop.");
    // The reset icon only appears once a shortcut differs from its default.
    expect(screen.queryByRole("button", { name: "Reset to default" })).not.toBeInTheDocument();
  });

  it("offers Cancel while capturing, which ends capture without changes", async () => {
    await userEvent.click(await renderPage());
    expect(screen.getByText("Esc or a click elsewhere cancels.")).toBeInTheDocument();

    // Esc ends capture here, so it is shown on the Cancel button as a key cap.
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveTextContent("Esc");

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(backend.commandsCalled("end_shortcut_capture")).toHaveLength(1);
    expect(screen.queryByText("Press the new shortcut…")).not.toBeInTheDocument();
    expect(field()).toHaveTextContent("Ctrl+Space");
  });

  it("shows the shortcut stored in the settings", async () => {
    backend.settings = { ...backend.settings, recordShortcut: "Ctrl+Win", shortcutMode: "toggle" };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);

    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Win/ }),
    ).toBeInTheDocument();
    const toggle = screen.getByRole("radio", { name: "Toggle Mode" });
    expect(toggle).toBeChecked();
    expect(toggle).toHaveAccessibleDescription("Press to start, press again to stop.");
  });

  // record-shortcut.md acceptance test 14: Ctrl down, Space down, Space up → "Ctrl+Space".
  it("captures a combination with a main key and saves it", async () => {
    await userEvent.click(await renderPage());
    expect(screen.getByText("Press the new shortcut…")).toBeInTheDocument();
    expect(backend.commandsCalled("begin_shortcut_capture")).toHaveLength(1);

    key("RightCtrl", true);
    key("LeftAlt", true);
    key("R", true);
    expect(field()).toHaveTextContent("Ctrl+Alt+R");
    key("R", false);

    expect(
      await screen.findByRole("button", { name: /Record Shortcut, currently Ctrl \+ Alt \+ R/ }),
    ).toBeInTheDocument();
    expect(backend.commandsCalled("set_record_shortcut")[0]?.args).toEqual({
      combination: "Ctrl+Alt+R",
    });
    expect(backend.settings.recordShortcut).toBe("Ctrl+Alt+R");
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

    // The description shows the capture hint while capturing, so click the label instead.
    await userEvent.click(screen.getByText("Record Shortcut"));

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

  // Issue #33: right Alt alone (AltGr) is a single modifier, rejected like any other.
  it("rejects right Alt alone with the single-modifier message", async () => {
    backend.rejectedShortcuts.set("Alt", { kind: "notAllowed", problem: "singleModifier" });
    await userEvent.click(await renderPage());

    key("RightAlt", true);
    key("RightAlt", false);

    expect(backend.commandsCalled("set_record_shortcut")[0]?.args).toEqual({
      combination: "Alt",
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Use at least two modifiers, e.g. Ctrl+Win, or a modifier with a key. Your previous shortcut stays active.",
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
      "Echo could not activate F9. Your previous shortcut stays active.",
    );
    expect(screen.getByText("Details: the keyboard hook is not running")).toHaveClass(
      "select-text",
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
    const reset = await resetButton("record");
    expect(reset).toBeEnabled();

    await userEvent.click(reset);

    expect(backend.commandsCalled("reset_setting")[0]?.args).toEqual({ key: "recordShortcut" });
    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Space/ }),
    ).toBeInTheDocument();
  });

  it("switches the mode and saves it", async () => {
    await renderPage();

    await userEvent.click(screen.getByRole("radio", { name: "Toggle Mode" }));

    expect(screen.getByRole("radio", { name: "Toggle Mode" })).toBeChecked();
    await waitFor(() => {
      expect(backend.settings.shortcutMode).toBe("toggle");
    });
  });

  it("follows a shortcut changed outside the window", async () => {
    await renderPage();

    act(() => {
      backend.changeSettings({ recordShortcut: "F9" });
    });

    expect(await screen.findByRole("button", { name: /currently F9/ })).toBeInTheDocument();
  });
});

describe("Cancel Shortcut settings", () => {
  function cancelField() {
    return screen.getByTestId("cancel-shortcut-field");
  }

  it("shows the Cancel Shortcut, Escape by default, with its description", async () => {
    await renderPage();

    expect(
      screen.getByRole("button", { name: "Change the Cancel Shortcut, currently Esc" }),
    ).toHaveTextContent("Esc");
    expect(
      screen.getByText(
        "Cancels the current Dictation. Active only while recording or transcribing.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reset to default" })).not.toBeInTheDocument();
  });

  it("captures a new Cancel Shortcut and saves it", async () => {
    await renderPage();
    await userEvent.click(cancelField());
    expect(screen.getByText("A click elsewhere cancels.")).toBeInTheDocument();
    // Esc can be the new value here, so Cancel does not offer it as the way out.
    expect(screen.getByRole("button", { name: "Cancel" })).not.toHaveTextContent("Esc");

    key("LeftCtrl", true);
    key("Q", true);
    key("Q", false);

    expect(
      await screen.findByRole("button", { name: "Change the Cancel Shortcut, currently Ctrl + Q" }),
    ).toBeInTheDocument();
    expect(backend.commandsCalled("set_cancel_shortcut")[0]?.args).toEqual({
      combination: "Ctrl+Q",
    });
    expect(backend.commandsCalled("set_record_shortcut")).toHaveLength(0);
    expect(field()).toHaveTextContent("Ctrl+Space");
  });

  // cancel-shortcut.md "UI": Escape alone is a value here and does not end the capture.
  it("accepts Escape alone as the Cancel Shortcut", async () => {
    backend.settings = { ...backend.settings, cancelShortcut: "F8" };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);
    await userEvent.click(
      await screen.findByRole("button", { name: /Cancel Shortcut, currently F8/ }),
    );

    key("Escape", true);
    expect(backend.commandsCalled("end_shortcut_capture")).toHaveLength(0);
    key("Escape", false);

    await waitFor(() => {
      expect(backend.settings.cancelShortcut).toBe("Escape");
    });
  });

  // Acceptance test 8 in the UI: the Record Shortcut is refused with a reason.
  it("explains why the Record Shortcut cannot be the Cancel Shortcut", async () => {
    backend.rejectedShortcuts.set("Ctrl+Space", { kind: "notAllowed", problem: "sameAsRecord" });
    await renderPage();
    await userEvent.click(cancelField());

    key("LeftCtrl", true);
    key("Space", true);
    key("Space", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ctrl + Space is already the Record Shortcut. Your previous shortcut stays active.",
    );
    expect(cancelField()).toHaveTextContent("Esc");
    expect(backend.settings.cancelShortcut).toBe("Escape");
  });

  it("resets to Escape", async () => {
    backend.settings = { ...backend.settings, cancelShortcut: "Ctrl+Q" };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);

    await userEvent.click(await resetButton("cancel"));

    expect(backend.commandsCalled("reset_setting")[0]?.args).toEqual({ key: "cancelShortcut" });
    expect(
      await screen.findByRole("button", { name: "Change the Cancel Shortcut, currently Esc" }),
    ).toBeInTheDocument();
  });

  it("is named in Polish", async () => {
    await renderPage();
    await act(() => changeUiLanguage("pl"));

    expect(
      await screen.findByRole("button", { name: /Zmień skrót anulowania, obecnie/ }),
    ).toBeInTheDocument();
    expect(screen.getByText("Skrót anulowania")).toBeInTheDocument();
  });
});

describe("Vocabulary Shortcut settings", () => {
  function vocabularyField() {
    return screen.getByTestId("vocabulary-shortcut-field");
  }

  function vocabularySwitch() {
    return screen.getByRole("switch", { name: "Add selected text to Vocabulary" });
  }

  it("sits below the Cancel Shortcut, on with Ctrl+Alt+D by default", async () => {
    await renderPage();

    const vocabulary = screen.getByRole("button", {
      name: "Change the Vocabulary Shortcut, currently Ctrl + Alt + D",
    });
    expect(vocabulary).toHaveTextContent("Ctrl+Alt+D");
    expect(vocabulary).toBeEnabled();
    expect(vocabularySwitch()).toBeChecked();
    expect(
      screen.getByText(
        "Select a name in any app and press the shortcut. Select just the name, without a Polish ending.",
      ),
    ).toBeInTheDocument();
    // vocabulary-shortcut.md "UI": the row comes right after the Cancel Shortcut.
    expect(
      screen.getByTestId("cancel-shortcut-field").compareDocumentPosition(vocabularyField()) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Reset to default" })).not.toBeInTheDocument();
  });

  it("turns the shortcut off and disables its capture field", async () => {
    await renderPage();

    await userEvent.click(vocabularySwitch());

    await waitFor(() => {
      expect(backend.settings.vocabularyShortcutEnabled).toBe(false);
    });
    expect(vocabularySwitch()).not.toBeChecked();
    expect(vocabularyField()).toBeDisabled();
    // The turned-off shortcut keeps its combination.
    expect(vocabularyField()).toHaveTextContent("Ctrl+Alt+D");

    await userEvent.click(vocabularyField());
    expect(backend.commandsCalled("begin_shortcut_capture")).toHaveLength(0);

    await userEvent.click(vocabularySwitch());
    await waitFor(() => {
      expect(backend.settings.vocabularyShortcutEnabled).toBe(true);
    });
    expect(vocabularyField()).toBeEnabled();
  });

  it("shows a turned-off shortcut from the settings", async () => {
    backend.settings = { ...backend.settings, vocabularyShortcutEnabled: false };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);

    expect(await screen.findByTestId("vocabulary-shortcut-field")).toBeDisabled();
    expect(vocabularySwitch()).not.toBeChecked();
  });

  it("captures a new Vocabulary Shortcut and saves it", async () => {
    await renderPage();
    await userEvent.click(vocabularyField());
    expect(screen.getByText("Esc or a click elsewhere cancels.")).toBeInTheDocument();
    // Esc ends this capture, as for the Record Shortcut.
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveTextContent("Esc");

    key("LeftCtrl", true);
    key("LeftShift", true);
    key("V", true);
    key("V", false);

    expect(
      await screen.findByRole("button", {
        name: "Change the Vocabulary Shortcut, currently Ctrl + Shift + V",
      }),
    ).toBeInTheDocument();
    expect(backend.commandsCalled("set_vocabulary_shortcut")[0]?.args).toEqual({
      combination: "Ctrl+Shift+V",
    });
    expect(backend.commandsCalled("set_record_shortcut")).toHaveLength(0);
    expect(backend.commandsCalled("set_cancel_shortcut")).toHaveLength(0);
  });

  it("ends capture without changes when Escape is pressed alone", async () => {
    await renderPage();
    await userEvent.click(vocabularyField());

    key("Escape", true);
    key("Escape", false);

    await waitFor(() => {
      expect(backend.commandsCalled("end_shortcut_capture")).toHaveLength(1);
    });
    expect(backend.commandsCalled("set_vocabulary_shortcut")).toHaveLength(0);
  });

  // Acceptance test 10, frontend half: a combination held by another shortcut is refused with a
  // message naming it, and the previous combination stays.
  it("explains why the Record Shortcut cannot be the Vocabulary Shortcut", async () => {
    backend.rejectedShortcuts.set("Ctrl+Space", { kind: "notAllowed", problem: "sameAsRecord" });
    await renderPage();
    await userEvent.click(vocabularyField());

    key("LeftCtrl", true);
    key("Space", true);
    key("Space", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ctrl + Space is already the Record Shortcut. Your previous shortcut stays active.",
    );
    expect(vocabularyField()).toHaveTextContent("Ctrl+Alt+D");
    expect(backend.settings.vocabularyShortcut).toBe("Ctrl+Alt+D");
  });

  it("explains why the Cancel Shortcut cannot be the Vocabulary Shortcut", async () => {
    backend.rejectedShortcuts.set("Ctrl+Q", { kind: "notAllowed", problem: "sameAsCancel" });
    await renderPage();
    await userEvent.click(vocabularyField());

    key("LeftCtrl", true);
    key("Q", true);
    key("Q", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ctrl + Q is already the Cancel Shortcut. Your previous shortcut stays active.",
    );
  });

  it("explains why the Vocabulary Shortcut cannot be the Record Shortcut", async () => {
    backend.rejectedShortcuts.set("Ctrl+Alt+D", {
      kind: "notAllowed",
      problem: "sameAsVocabulary",
    });
    await renderPage();
    await userEvent.click(field());

    key("LeftCtrl", true);
    key("LeftAlt", true);
    key("D", true);
    key("D", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ctrl + Alt + D is already the Vocabulary Shortcut. Your previous shortcut stays active.",
    );
    expect(field()).toHaveTextContent("Ctrl+Space");
  });

  it("explains why the Vocabulary Shortcut cannot be the Cancel Shortcut", async () => {
    backend.rejectedShortcuts.set("Ctrl+Alt+D", {
      kind: "notAllowed",
      problem: "sameAsVocabulary",
    });
    await renderPage();
    await userEvent.click(screen.getByTestId("cancel-shortcut-field"));

    key("LeftCtrl", true);
    key("LeftAlt", true);
    key("D", true);
    key("D", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ctrl + Alt + D is already the Vocabulary Shortcut. Your previous shortcut stays active.",
    );
  });

  it("explains why a text-editing combination is refused", async () => {
    backend.rejectedShortcuts.set("Ctrl+C", { kind: "notAllowed", problem: "editingCombination" });
    await renderPage();
    await userEvent.click(vocabularyField());

    key("LeftCtrl", true);
    key("C", true);
    key("C", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Applications need Ctrl + C for editing text. Your previous shortcut stays active.",
    );
    expect(vocabularyField()).toHaveTextContent("Ctrl+Alt+D");
  });

  it("resets to Ctrl+Alt+D", async () => {
    backend.settings = { ...backend.settings, vocabularyShortcut: "Ctrl+Shift+V" };
    await useSettings.getState().load();
    render(<RecordShortcutSettings />);

    await userEvent.click(await resetButton("vocabulary"));

    expect(backend.commandsCalled("reset_setting")[0]?.args).toEqual({
      key: "vocabularyShortcut",
    });
    expect(
      await screen.findByRole("button", {
        name: "Change the Vocabulary Shortcut, currently Ctrl + Alt + D",
      }),
    ).toBeInTheDocument();
  });

  it("is named in Polish", async () => {
    await renderPage();
    await act(() => changeUiLanguage("pl"));

    expect(
      await screen.findByRole("switch", { name: "Dodaj zaznaczony tekst do Słownika" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Zaznacz nazwę w dowolnej aplikacji i naciśnij skrót. Zaznacz samą nazwę, bez końcówki.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Zmień skrót dodawania do Słownika, obecnie/ }),
    ).toBeInTheDocument();
  });
});
