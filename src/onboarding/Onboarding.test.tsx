import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { DEFAULT_SETTINGS, backend } from "../test/backend";
import type { MicrophoneAccess } from "./steps";

const microphone = vi.hoisted(() => {
  const state: { access: MicrophoneAccess; checks: number } = { access: "allowed", checks: 0 };
  return state;
});
vi.mock("./sources", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./sources")>()),
  checkMicrophoneAccess: () => {
    microphone.checks += 1;
    return Promise.resolve(microphone.access);
  },
}));

beforeEach(async () => {
  microphone.access = "allowed";
  microphone.checks = 0;
  backend.settings = {
    ...DEFAULT_SETTINGS,
    uiLanguage: "en",
    onboardingWelcomeDone: false,
    onboardingCompleted: false,
  };
  await changeUiLanguage("en");
});

afterEach(() => {
  vi.useRealTimers();
});

describe("onboarding", () => {
  // settings-and-first-run.md acceptance test 1 (frontend part).
  it("opens on Welcome with fresh settings", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { level: 1, name: "Welcome to Echo" })).toBeVisible();
    expect(screen.queryByRole("navigation", { name: "Sections" })).not.toBeInTheDocument();
  });

  it("switches the UI Language on Welcome and saves it", async () => {
    render(<App />);

    await userEvent.click(await screen.findByRole("radio", { name: "Polski" }));

    expect(
      await screen.findByRole("heading", { level: 1, name: "Witamy w Echo" }),
    ).toBeInTheDocument();
    expect(backend.settings.uiLanguage).toBe("pl");
    expect(document.documentElement.lang).toBe("pl");
  });

  // Acceptance test 2, "allowed" half.
  it("skips Microphone access when Windows allows the Microphone", async () => {
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Get started" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
    expect(backend.settings.onboardingWelcomeDone).toBe(true);
  });

  // Acceptance test 2, "denied" half.
  it("shows Microphone access while blocked and continues once Windows allows it", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    microphone.access = "denied";
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    expect(
      await screen.findByRole("heading", { level: 1, name: "Allow microphone access" }),
    ).toBeVisible();
    microphone.access = "allowed";
    await act(() => vi.advanceTimersByTimeAsync(2000));

    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
  });

  it("re-checks Microphone access when the window regains focus", async () => {
    microphone.access = "denied";
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Allow microphone access" });

    microphone.access = "allowed";
    const before = microphone.checks;
    act(() => {
      fireEvent.focus(window);
    });

    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
    expect(microphone.checks).toBeGreaterThan(before);
  });

  it("opens the Windows privacy settings and can be skipped", async () => {
    microphone.access = "denied";
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    await userEvent.click(
      await screen.findByRole("button", { name: "Open Windows privacy settings" }),
    );
    expect(backend.commandsCalled("open_microphone_privacy_settings")).toHaveLength(1);

    await userEvent.click(screen.getByRole("button", { name: "Skip for now" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
  });

  // Rule 4: onboarding resumes at the first incomplete step.
  it("resumes at Choose a Model after Welcome was completed earlier", async () => {
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
  });

  // Acceptance test 3: Download on the recommended Model; once it is active, "Try it" follows.
  it("downloads the recommended Model and continues to Try it once it is active", async () => {
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    const recommended = await screen.findByRole("radio", { name: /Whisper large-v3-turbo/ });
    expect(recommended).toBeChecked();
    expect(screen.getAllByRole("radio")).toHaveLength(3);
    await userEvent.click(screen.getByRole("button", { name: "Download (845 MB)" }));
    expect(backend.commandsCalled("download_model")).toEqual([
      { command: "download_model", args: { model: "whisperLargeV3Turbo" } },
    ]);

    act(() => {
      backend.changeModel("whisperLargeV3Turbo", {
        download: {
          state: "downloading",
          downloaded: 443_190_880,
          total: 886_381_760,
          bytesPerSecond: 12_582_912,
        },
      });
    });
    expect(await screen.findByText("50% · 12.0 MB/s")).toBeVisible();
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(backend.commandsCalled("cancel_model_download")).toHaveLength(1);

    act(() => {
      backend.changeModel("whisperLargeV3Turbo", { downloaded: true, download: { state: "idle" } });
      backend.changeModels({ active: "whisperLargeV3Turbo", activeState: "ready" });
    });

    expect(await screen.findByRole("heading", { level: 1, name: "Try it" })).toBeVisible();
  });

  it("offers Use this Model for a Model downloaded earlier", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.changeModel("whisperSmall", { downloaded: true });
    render(<App />);

    await userEvent.click(await screen.findByRole("radio", { name: /Whisper small/ }));
    await userEvent.click(screen.getByRole("button", { name: "Use this Model" }));

    expect(backend.commandsCalled("activate_model")).toEqual([
      { command: "activate_model", args: { model: "whisperSmall" } },
    ]);
    expect(screen.queryByRole("button", { name: /Continue without/ })).not.toBeInTheDocument();
  });

  it("shows the Record Shortcut on Try it and finishes into the main settings", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    expect(await screen.findByRole("heading", { level: 1, name: "Try it" })).toBeVisible();
    expect(screen.getByText("Ctrl+Space").closest("p")).toHaveTextContent(
      "Hold Ctrl+Space and speak",
    );
    expect(screen.getByRole("textbox", { name: "Test field" })).toBeVisible();

    await userEvent.click(screen.getByRole("button", { name: "Finish" }));

    expect(await screen.findByRole("navigation", { name: "Sections" })).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(true);
  });

  // Acceptance test 5 (frontend part).
  it("is not shown again once completed", async () => {
    backend.settings.onboardingCompleted = true;
    render(<App />);

    expect(await screen.findByRole("navigation", { name: "Sections" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Welcome to Echo" })).not.toBeInTheDocument();
  });
});
