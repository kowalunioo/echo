import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import type { DictationProblem } from "../bindings";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

const transcription: DictationProblem = {
  id: 1,
  kind: "transcriptionFailed",
  detail: "GPU lost",
};
const noModel: DictationProblem = { id: 2, kind: "noModel", detail: "" };

beforeEach(async () => {
  useShell.setState(initialShell, true);
  await changeUiLanguage("en");
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Dictation error notices", () => {
  it("shows nothing without errors", async () => {
    render(<App />);
    await screen.findByRole("heading", { level: 1 });
    expect(screen.queryByRole("alert", { name: "Dictation problems" })).not.toBeInTheDocument();
  });

  it("lists the errors queued before the window opened, newest first", async () => {
    backend.dictation = {
      ...backend.dictation,
      error: noModel,
      notices: [transcription, noModel],
    };
    render(<App />);

    const notices = await screen.findByRole("alert", { name: "Dictation problems" });
    const items = within(notices).getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("No Model — download one to start dictating.");
    expect(items[1]).toHaveTextContent("Transcription failed.");
    expect(items[1]).toHaveTextContent("Details: GPU lost");
  });

  it("shows an error that happens while the window is open", async () => {
    render(<App />);
    await screen.findByRole("heading", { level: 1 });

    act(() => {
      backend.changeDictation({ error: transcription, notices: [transcription] });
    });

    expect(await screen.findByText("Transcription failed.")).toBeVisible();
  });

  it("clears the tray error once the window is visible and focused, keeping the notice", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    backend.dictation = { ...backend.dictation, error: transcription, notices: [transcription] };
    render(<App />);

    await screen.findByText("Transcription failed.");
    await vi.waitFor(() => {
      expect(backend.calls.some((c) => c.command === "dictation_window_seen")).toBe(true);
    });
    expect(backend.dictation.error).toBeNull();
    expect(screen.getByText("Transcription failed.")).toBeVisible();
  });

  it("does not clear the tray error while the window is not focused", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    backend.dictation = { ...backend.dictation, error: transcription, notices: [transcription] };
    render(<App />);

    await screen.findByText("Transcription failed.");
    expect(backend.calls.some((c) => c.command === "dictation_window_seen")).toBe(false);
  });

  it("dismisses the notices", async () => {
    backend.dictation = { ...backend.dictation, notices: [transcription] };
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Dismiss" }));

    expect(screen.queryByText("Transcription failed.")).not.toBeInTheDocument();
  });

  it("opens the Models page from a missing-Model notice", async () => {
    backend.dictation = { ...backend.dictation, notices: [noModel] };
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Open Models" }));

    expect(useShell.getState().page).toBe("model");
  });

  it("opens the Windows privacy settings from a blocked-microphone notice", async () => {
    backend.dictation = {
      ...backend.dictation,
      notices: [{ id: 3, kind: "microphoneAccessDenied", detail: "" }],
    };
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Open privacy settings" }));

    expect(backend.calls.some((c) => c.command === "open_microphone_privacy_settings")).toBe(true);
  });

  it("is translated into Polish", async () => {
    backend.settings.uiLanguage = "pl";
    backend.dictation = { ...backend.dictation, notices: [transcription] };
    render(<App />);

    expect(await screen.findByText("Transkrypcja nie powiodła się.")).toBeVisible();
    expect(screen.getByRole("button", { name: "Zamknij" })).toBeVisible();
  });
});
