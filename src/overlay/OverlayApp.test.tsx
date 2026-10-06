import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type OverlayView, commands } from "../bindings";
import { changeUiLanguage } from "../i18n";
import { backend } from "../test/backend";
import { OverlayApp } from "./OverlayApp";

afterEach(async () => {
  vi.restoreAllMocks();
  await changeUiLanguage("en");
});

function pill() {
  return screen.getByTestId("overlay-pill");
}

function frame(level: number, elapsedMs: number) {
  act(() => {
    backend.emit("overlay-frame", { level, elapsedMs });
  });
}

describe("Overlay", () => {
  // Rule 1.
  it("starts hidden", async () => {
    render(<OverlayApp />);
    await act(async () => {});
    expect(pill()).toHaveAttribute("data-visible", "false");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  // Rule 2.
  it("shows getting ready with a muted meter until audio flows", async () => {
    backend.overlay = { kind: "gettingReady" };
    render(<OverlayApp />);
    expect(await screen.findByRole("status")).toHaveTextContent("Getting ready…");
    expect(pill()).toHaveAttribute("data-visible", "true");
    expect(screen.getByTestId("overlay-meter").children).toHaveLength(9);
    expect(screen.getByRole("button", { name: "Cancel Dictation" })).toBeVisible();
  });

  // Rule 3.
  it("shows listening with a live meter, a timer and cancel", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({ kind: "listening" });
    });
    expect(await screen.findByRole("status")).toHaveTextContent("Listening");
    expect(screen.getByText("0:00")).toBeVisible();
    const bar = () => screen.getByTestId("overlay-meter").children[4] as HTMLElement;
    const silent = bar().style.height;
    frame(1, 3_400);
    expect(screen.getByText("0:03")).toBeVisible();
    expect(bar().style.height).not.toBe(silent);

    await userEvent.click(screen.getByRole("button", { name: "Cancel Dictation" }));
    expect(backend.calls.map((c) => c.command)).toContain("overlay_cancel");
  });

  // Rule 4: cancel while Transcribing, not once Inserting starts.
  it("shows transcribing with cancel only until Insertion starts", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({ kind: "transcribing", cancellable: true });
    });
    expect(await screen.findByRole("status")).toHaveTextContent("Transcribing…");
    expect(screen.getByRole("button", { name: "Cancel Dictation" })).toBeVisible();
    act(() => {
      backend.changeOverlay({ kind: "transcribing", cancellable: false });
    });
    expect(screen.queryByRole("button", { name: "Cancel Dictation" })).not.toBeInTheDocument();
  });

  // Rule 5.
  it("shows a message, clickable when it has an action", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({
        kind: "message",
        message: { kind: "problem", problem: "transcriptionFailed" },
        actionable: false,
      });
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("Transcription failed");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();

    act(() => {
      backend.changeOverlay({
        kind: "message",
        message: { kind: "problem", problem: "noModel" },
        actionable: true,
      });
    });
    const message = await screen.findByRole("alert");
    expect(message).toHaveTextContent("No Model — open Echo to download one");
    await userEvent.click(message);
    expect(backend.calls.map((c) => c.command)).toContain("overlay_message_clicked");
  });

  it("shows the microphone fallback notice", async () => {
    backend.overlay = {
      kind: "message",
      message: { kind: "microphoneFallback" },
      actionable: false,
    };
    render(<OverlayApp />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Selected microphone not found — using the default microphone",
    );
  });

  // Rule 6: the content stays while the pill fades out.
  it("fades out showing what it showed last", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({ kind: "transcribing", cancellable: false });
    });
    expect(await screen.findByRole("status")).toHaveTextContent("Transcribing…");
    act(() => {
      backend.changeOverlay({ kind: "hidden" });
    });
    expect(pill()).toHaveAttribute("data-visible", "false");
    expect(screen.getByRole("status")).toHaveTextContent("Transcribing…");
  });

  it("keeps a change that arrives before the first view is fetched", async () => {
    let answer: (view: OverlayView) => void = () => undefined;
    const fetched = vi.spyOn(commands, "getOverlayView").mockReturnValue(
      new Promise((resolve) => {
        answer = resolve;
      }),
    );
    render(<OverlayApp />);
    await act(async () => {});
    act(() => {
      backend.changeOverlay({ kind: "transcribing", cancellable: false });
    });
    expect(await screen.findByRole("status")).toHaveTextContent("Transcribing…");

    // The stale answer (from before the change) arrives late and must not win.
    await act(async () => {
      answer({ kind: "hidden" });
      await Promise.resolve();
    });
    expect(pill()).toHaveAttribute("data-visible", "true");
    expect(fetched).toHaveBeenCalledOnce();
  });

  // Rule 18.
  it("follows the UI Language", async () => {
    await changeUiLanguage("pl");
    backend.overlay = { kind: "transcribing", cancellable: true };
    render(<OverlayApp />);
    expect(await screen.findByRole("status")).toHaveTextContent("Transkrybuję…");
    expect(screen.getByRole("button", { name: "Anuluj dyktowanie" })).toBeVisible();
  });
});
