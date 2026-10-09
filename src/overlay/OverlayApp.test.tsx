import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type OverlayView, commands } from "../bindings";
import { changeUiLanguage, i18n } from "../i18n";
import { backend } from "../test/backend";
import { OverlayApp } from "./OverlayApp";

afterEach(async () => {
  vi.restoreAllMocks();
  await changeUiLanguage("en");
});

/**
 * Measured in Segoe UI Variable Text 13 px medium (WebView2), the longest message (60 characters)
 * is 382 px; a clickable message has 436 px for its text in the 520 px window.
 */
const MAX_MESSAGE_LENGTH = 60;

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

  // Rule 2: the label is on screen, so nobody starts talking before the Microphone is live.
  it("says it is getting ready, with no meter or timer, until audio flows", async () => {
    backend.overlay = { kind: "gettingReady" };
    render(<OverlayApp />);
    const status = await screen.findByRole("status");
    expect(status).toHaveTextContent("Getting ready…");
    expect(screen.getByTestId("overlay-label")).toHaveTextContent("Getting ready…");
    expect(screen.getByTestId("overlay-label")).toHaveAttribute("data-shown", "true");
    expect(pill()).toHaveAttribute("data-visible", "true");
    expect(screen.queryByTestId("overlay-meter")).not.toBeInTheDocument();
    expect(screen.queryByText("0:00")).not.toBeInTheDocument();
    expect(screen.getByTestId("overlay-mark")).toHaveAttribute("data-live", "false");
    expect(screen.getByRole("button", { name: "Cancel Dictation" })).toBeVisible();
  });

  // Rule 3. No timer: the meter already shows the Recording is live (overlay.md, Decisions).
  it("shows listening with a live meter and cancel, and no timer", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({ kind: "gettingReady" });
    });
    await screen.findByRole("status");
    act(() => {
      backend.changeOverlay({ kind: "listening" });
    });
    // The same live region now says "Listening"; the visible label fades out as the meter fades in.
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("Listening");
    expect(status).toHaveClass("sr-only");
    expect(screen.getByTestId("overlay-label")).toHaveAttribute("data-shown", "false");
    expect(screen.getByTestId("overlay-mark")).toHaveAttribute("data-live", "true");
    // Thirteen bars; a sound shows first in the centre one (meter.ts).
    expect(screen.getByTestId("overlay-meter").children).toHaveLength(13);
    const bar = () => screen.getByTestId("overlay-meter").children[6] as HTMLElement;
    const silent = bar().style.transform;
    frame(1, 3_400);
    expect(bar().style.transform).not.toBe(silent);
    expect(screen.queryByText("0:03")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Cancel Dictation" }));
    expect(backend.calls.map((c) => c.command)).toContain("overlay_cancel");
  });

  // UI: the recording states carry the logo's three bars; muted until the Microphone is live.
  it("shows the logo mark while recording", async () => {
    backend.overlay = { kind: "listening" };
    render(<OverlayApp />);
    const mark = await screen.findByTestId("overlay-mark");
    expect(mark.querySelectorAll("rect")).toHaveLength(3);
  });

  // UI: the pill's edge is a light outline that shimmers while getting ready; one edge, no border.
  it("draws the pill's edge as one light outline", async () => {
    backend.overlay = { kind: "gettingReady" };
    render(<OverlayApp />);
    await screen.findByRole("status");
    expect(screen.getAllByTestId("overlay-edge")).toHaveLength(1);
    expect(pill()).toHaveAttribute("data-phase", "gettingReady");
  });

  // Rule 5: every message fits on one line in the Overlay window.
  it.each(["en", "pl"] as const)("keeps every %s message short enough for one line", (lng) => {
    const messages = i18n.getResourceBundle(lng, "translation") as {
      overlay: { messages: Record<string, string> };
    };
    for (const text of Object.values(messages.overlay.messages)) {
      expect(text.length, text).toBeLessThanOrEqual(MAX_MESSAGE_LENGTH);
    }
  });

  // Rule 4: the pill becomes three dots; Transcribing is announced but has no button (the
  // Cancel Shortcut still cancels it).
  it("shows transcribing as three dots with no cancel button", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({ kind: "transcribing", cancellable: true });
    });
    const status = await screen.findByRole("status");
    expect(status).toHaveTextContent("Transcribing…");
    expect(status).toHaveClass("sr-only");
    expect(pill()).toHaveAttribute("data-phase", "transcribing");
    expect(screen.getByTestId("overlay-dots").children).toHaveLength(3);
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
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "No Model — open Echo to download one",
    );
    // The clickable message keeps its button semantics; the alert is the text inside it.
    const button = screen.getByRole("button", { name: "No Model — open Echo to download one" });
    await userEvent.click(button);
    expect(backend.calls.map((c) => c.command)).toContain("overlay_message_clicked");
  });

  // dictation-pipeline.md rule 36 with a History limit of 0.
  it("points to Echo to copy text that could not be inserted", async () => {
    render(<OverlayApp />);
    act(() => {
      backend.changeOverlay({
        kind: "message",
        message: { kind: "problem", problem: "insertionFailedNotInHistory" },
        actionable: true,
      });
    });
    expect(
      await screen.findByRole("button", {
        name: "Couldn't insert the text — open Echo to copy it",
      }),
    ).toBeVisible();
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
    backend.overlay = { kind: "gettingReady" };
    render(<OverlayApp />);
    expect(await screen.findByRole("status")).toHaveTextContent("Przygotowuję…");
    expect(screen.getByRole("button", { name: "Anuluj dyktowanie" })).toBeVisible();
    act(() => {
      backend.changeOverlay({ kind: "transcribing", cancellable: false });
    });
    expect(screen.getByRole("status")).toHaveTextContent("Transkrybuję…");
  });

  // dictation-pipeline.md rule 41: a fake-microphone session is marked in the Overlay too.
  it("marks fake-microphone mode in the pill while it is shown", async () => {
    backend.testAudio = "sample.wav";
    backend.overlay = { kind: "listening" };
    render(<OverlayApp />);
    const marker = await screen.findByRole("status", { name: "Test audio" });
    expect(marker).toHaveTextContent("TEST AUDIO");
    expect(pill()).toContainElement(marker);
  });

  it("shows no test-audio marker in normal operation", async () => {
    backend.overlay = { kind: "listening" };
    render(<OverlayApp />);
    expect(await screen.findByRole("status")).toHaveTextContent("Listening");
    expect(screen.queryByRole("status", { name: "Test audio" })).not.toBeInTheDocument();
  });
});
