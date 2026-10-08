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
    expect(screen.queryByRole("region", { name: "Dictation problems" })).not.toBeInTheDocument();
  });

  it("lists the errors queued before the window opened, newest first", async () => {
    backend.dictation = {
      ...backend.dictation,
      error: noModel,
      notices: [transcription, noModel],
    };
    render(<App />);

    const notices = await screen.findByRole("region", { name: "Dictation problems" });
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

    await userEvent.click(await screen.findByRole("button", { name: "Open Model settings" }));

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

  it("announces the message line, not the section with its buttons", async () => {
    backend.dictation = { ...backend.dictation, notices: [transcription] };
    render(<App />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Transcription failed.");
    expect(within(alert).queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows the technical detail below the message as selectable text", async () => {
    backend.dictation = { ...backend.dictation, notices: [transcription] };
    render(<App />);

    const detail = await screen.findByText("Details: GPU lost");
    expect(detail).toHaveClass("select-text");
    expect(screen.getByRole("alert")).not.toHaveTextContent("GPU lost");
  });

  it.each(["modelLoadFailed", "modelDownloadFailed"] as const)(
    "opens the Models page from a %s notice",
    async (kind) => {
      backend.dictation = {
        ...backend.dictation,
        notices: [{ id: 4, kind, detail: "Whisper small: out of memory" }],
      };
      render(<App />);

      await userEvent.click(await screen.findByRole("button", { name: "Open Model settings" }));

      expect(useShell.getState().page).toBe("model");
    },
  );

  it.each(["microphoneNotFound", "microphoneFailed", "microphoneDisconnected"] as const)(
    "goes to the Microphone setting from a %s notice",
    async (kind) => {
      useShell.setState({ page: "history" });
      backend.dictation = { ...backend.dictation, notices: [{ id: 5, kind, detail: "" }] };
      render(<App />);

      await userEvent.click(
        await screen.findByRole("button", { name: "Open Microphone settings" }),
      );

      expect(useShell.getState().page).toBe("dictation");
      await vi.waitFor(() => {
        expect(screen.getByRole("region", { name: "Microphone" })).toHaveFocus();
      });
    },
  );

  it("opens the log folder from a transcription notice", async () => {
    backend.dictation = { ...backend.dictation, notices: [transcription] };
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Open log folder" }));

    expect(backend.calls.some((c) => c.command === "open_log_folder")).toBe(true);
  });

  // dictation-pipeline.md rule 36, with History on.
  it("offers Copy text and Open History when Insertion failed", async () => {
    const failed: DictationProblem = { id: 6, kind: "insertionFailed", detail: "blocked" };
    backend.keptTranscript = "Ala ma kota";
    backend.dictation = { ...backend.dictation, notices: [failed], keptTranscript: 6 };
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByText("Couldn't insert the text — it is in History.")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Copy text" }));

    expect(await navigator.clipboard.readText()).toBe("Ala ma kota");
    expect(screen.getByRole("button", { name: "Copied" })).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Open History" }));
    expect(useShell.getState().page).toBe("history");
  });

  // dictation-pipeline.md rule 36, with a History limit of 0 (history.md rule 9).
  it("does not claim History holds the text when History is off", async () => {
    const failed: DictationProblem = {
      id: 7,
      kind: "insertionFailedNotInHistory",
      detail: "blocked",
    };
    backend.keptTranscript = "Ala ma kota";
    backend.dictation = { ...backend.dictation, notices: [failed], keptTranscript: 7 };
    const user = userEvent.setup();
    render(<App />);

    const message = await screen.findByText(
      "Couldn't insert the text. Copy it before your next Dictation.",
    );
    expect(message).toBeVisible();
    const notices = screen.getByRole("region", { name: "Dictation problems" });
    expect(notices).not.toHaveTextContent("History");
    expect(screen.queryByRole("button", { name: "Open History" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Copy text" }));
    expect(await navigator.clipboard.readText()).toBe("Ala ma kota");
  });

  it("no longer offers Copy text once the next Recording has started", async () => {
    const failed: DictationProblem = { id: 8, kind: "insertionFailed", detail: "" };
    backend.dictation = { ...backend.dictation, notices: [failed], keptTranscript: 8 };
    render(<App />);

    expect(await screen.findByRole("button", { name: "Copy text" })).toBeVisible();
    act(() => {
      backend.changeDictation({ keptTranscript: null });
    });

    expect(screen.queryByRole("button", { name: "Copy text" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open History" })).toBeVisible();
  });

  it("is translated into Polish", async () => {
    backend.settings.uiLanguage = "pl";
    backend.dictation = { ...backend.dictation, notices: [transcription] };
    render(<App />);

    expect(await screen.findByText("Transkrypcja nie powiodła się.")).toBeVisible();
    expect(screen.getByRole("button", { name: "Zamknij" })).toBeVisible();
  });
});
