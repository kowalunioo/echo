import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CapturedKeyEvent, RecordIntentEvent, RecordShortcutState } from "../bindings";
import { changeUiLanguage, initI18n } from "../i18n";
import { useRecordShortcut } from "../store/recordShortcut";
import { RecordShortcutSettings } from "./RecordShortcutSettings";

// The generated bindings call Tauri's `invoke` and `listen`; faking them exercises the real
// binding code.
const invoke = vi.hoisted(() => vi.fn());
const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler);
    return Promise.resolve(() => listeners.delete(name));
  },
}));

const initialStore = useRecordShortcut.getState();
initI18n("en");

let backend: RecordShortcutState;

/** A fake backend: accepts any proposal except the ones the test lists as rejected. */
function fakeBackend(rejected: Record<string, unknown> = {}) {
  backend = { combination: "Ctrl+Space", mode: "pushToTalk", defaultCombination: "Ctrl+Space" };
  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "record_shortcut":
      case "begin_shortcut_capture":
      case "end_shortcut_capture":
        return Promise.resolve(command === "record_shortcut" ? backend : null);
      case "set_record_shortcut": {
        const combination = args?.combination as string;
        // Tauri rejects with the command's serialised error value, not an Error.
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors
        if (combination in rejected) return Promise.reject(rejected[combination]);
        backend = { ...backend, combination };
        return Promise.resolve(backend);
      }
      case "reset_record_shortcut":
        backend = { ...backend, combination: backend.defaultCombination };
        return Promise.resolve(backend);
      case "set_shortcut_mode":
        backend = { ...backend, mode: args?.mode as RecordShortcutState["mode"] };
        return Promise.resolve(backend);
      default:
        return Promise.reject(new Error(`unexpected ${command}`));
    }
  });
}

function emit(name: string, payload: CapturedKeyEvent | RecordIntentEvent) {
  act(() => {
    listeners.get(name)?.({ payload });
  });
}

function key(key: string, pressed: boolean) {
  emit("captured-key-event", { key, pressed });
}

async function renderPage() {
  render(<RecordShortcutSettings />);
  return screen.findByRole("button", { name: /currently Ctrl \+ Space/ });
}

beforeEach(async () => {
  invoke.mockReset();
  listeners.clear();
  useRecordShortcut.setState(initialStore, true);
  await changeUiLanguage("en");
});

describe("Record Shortcut settings", () => {
  it("shows the current shortcut as key caps and the default mode", async () => {
    fakeBackend();
    const field = await renderPage();

    expect(field).toHaveTextContent("Ctrl+Space");
    expect(screen.getByRole("radio", { name: /Hold to record/ })).toBeChecked();
    expect(screen.getByRole("button", { name: "Reset to default" })).toBeDisabled();
  });

  // record-shortcut.md acceptance test 14: Ctrl down, Space down, Space up → "Ctrl+Space".
  it("captures a combination with a main key", async () => {
    fakeBackend();
    await userEvent.click(await renderPage());
    expect(screen.getByText("Press the new shortcut…")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("begin_shortcut_capture");

    key("RightCtrl", true);
    key("LeftAlt", true);
    key("D", true);
    expect(screen.getByTestId("record-shortcut-field")).toHaveTextContent("Ctrl+Alt+D");
    key("D", false);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("set_record_shortcut", { combination: "Ctrl+Alt+D" });
    });
    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Alt \+ D/ }),
    ).toBeInTheDocument();
  });

  // Acceptance test 14: Ctrl down, Win down, Win up, Ctrl up → "Ctrl+Win".
  it("captures a modifier-only combination when all modifiers are released", async () => {
    fakeBackend();
    await userEvent.click(await renderPage());

    key("LeftCtrl", true);
    key("LeftWin", true);
    key("LeftWin", false);
    expect(invoke).not.toHaveBeenCalledWith("set_record_shortcut", expect.anything());
    key("LeftCtrl", false);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("set_record_shortcut", { combination: "Ctrl+Win" });
    });
  });

  // Acceptance test 14: click outside → no change.
  it("ends capture without changes on a click outside the field", async () => {
    fakeBackend();
    await userEvent.click(await renderPage());
    key("LeftCtrl", true);

    await userEvent.click(screen.getByText("Starts and stops a Recording from any application."));

    expect(invoke).toHaveBeenCalledWith("end_shortcut_capture");
    expect(invoke).not.toHaveBeenCalledWith("set_record_shortcut", expect.anything());
    expect(screen.getByTestId("record-shortcut-field")).toHaveTextContent("Ctrl+Space");
  });

  it("ends capture without changes when the window loses focus", async () => {
    fakeBackend();
    await userEvent.click(await renderPage());

    act(() => {
      window.dispatchEvent(new Event("blur"));
    });

    expect(invoke).toHaveBeenCalledWith("end_shortcut_capture");
    expect(screen.queryByText("Press the new shortcut…")).not.toBeInTheDocument();
  });

  it("ends capture without changes when Escape is pressed alone", async () => {
    fakeBackend();
    await userEvent.click(await renderPage());

    key("Escape", true);

    expect(invoke).toHaveBeenCalledWith("end_shortcut_capture");
    expect(invoke).not.toHaveBeenCalledWith("set_record_shortcut", expect.anything());
  });

  it("explains a rejected proposal and keeps the previous shortcut", async () => {
    fakeBackend({ Space: { kind: "notAllowed", problem: "needsModifier" } });
    await userEvent.click(await renderPage());

    key("Space", true);
    key("Space", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Space alone would stop working for typing. Add Ctrl, Alt, Shift or Win. Your previous shortcut stays active.",
    );
    expect(screen.getByTestId("record-shortcut-field")).toHaveTextContent("Ctrl+Space");
  });

  it("shows why an allowed shortcut could not be activated", async () => {
    fakeBackend({ F9: { kind: "activationFailed", reason: "the keyboard hook is not running" } });
    await userEvent.click(await renderPage());

    key("F9", true);
    key("F9", false);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Echo could not activate F9: the keyboard hook is not running",
    );
  });

  it("names keys in the UI Language", async () => {
    fakeBackend();
    await renderPage();
    await act(() => changeUiLanguage("pl"));

    expect(
      await screen.findByRole("button", { name: "Zmień skrót nagrywania, obecnie Ctrl + Spacja" }),
    ).toBeInTheDocument();
  });

  it("resets to the default", async () => {
    fakeBackend();
    await userEvent.click(await renderPage());
    key("F9", true);
    key("F9", false);
    const reset = screen.getByRole("button", { name: "Reset to default" });
    await waitFor(() => {
      expect(reset).toBeEnabled();
    });

    await userEvent.click(reset);

    expect(invoke).toHaveBeenCalledWith("reset_record_shortcut");
    expect(
      await screen.findByRole("button", { name: /currently Ctrl \+ Space/ }),
    ).toBeInTheDocument();
  });

  it("switches the mode", async () => {
    fakeBackend();
    await renderPage();

    await userEvent.click(screen.getByRole("radio", { name: /Press to start, press again/ }));

    expect(invoke).toHaveBeenCalledWith("set_shortcut_mode", { mode: "toggle" });
    expect(screen.getByRole("radio", { name: /Press to start, press again/ })).toBeChecked();
  });

  it("shows the last Record Shortcut intent in the temporary developer check", async () => {
    fakeBackend();
    await renderPage();
    expect(screen.getByTestId("dev-intent")).toHaveTextContent("none yet");

    emit("record-intent-event", { intent: "start" });
    expect(screen.getByTestId("dev-intent")).toHaveTextContent("start");
    emit("record-intent-event", { intent: "stop" });
    expect(screen.getByTestId("dev-intent")).toHaveTextContent("stop");
  });
});
