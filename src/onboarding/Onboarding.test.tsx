import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
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
  useShell.setState({ page: "dictation", section: null });
  backend.computeHardware = "gpu";
  backend.settings = {
    ...DEFAULT_SETTINGS,
    uiLanguage: "en",
    onboardingWelcomeDone: false,
    onboardingCompleted: false,
    microphone: { kind: "default" },
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

  // Rule 2.1: Welcome names no sizes, since the catalogue of Models will grow; each Model's size
  // shows on the next step.
  it("says a Model is downloaded once without naming sizes on Welcome", async () => {
    render(<App />);

    expect(
      await screen.findByText(
        "Echo needs a speech Model, downloaded once. You choose it in the next step.",
      ),
    ).toBeVisible();
    expect(screen.queryByText(/MB/)).not.toBeInTheDocument();
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

    // Every Model is a row with its own Download (no radio to pick first), so several can be tried.
    const list = await screen.findByRole("list", { name: "Models" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);
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
    await userEvent.click(
      screen.getByRole("button", { name: "Cancel the download of Whisper large-v3-turbo" }),
    );
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

    await userEvent.click(await screen.findByRole("button", { name: "Use this Model" }));

    expect(backend.commandsCalled("activate_model")).toEqual([
      { command: "activate_model", args: { model: "whisperSmall" } },
    ]);
    expect(screen.queryByRole("button", { name: /Continue without/ })).not.toBeInTheDocument();
  });

  // models.md rule 7: a second Download while one runs queues it, so the user can try several.
  it("queues a second Model's download while the first one runs", async () => {
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Download (845 MB)" }));
    act(() => {
      backend.changeModel("whisperLargeV3Turbo", {
        download: { state: "downloading", downloaded: 0, total: 886_381_760, bytesPerSecond: 0 },
      });
    });
    await userEvent.click(screen.getByRole("button", { name: "Download (257 MB)" }));

    expect(backend.commandsCalled("download_model")).toEqual([
      { command: "download_model", args: { model: "whisperLargeV3Turbo" } },
      { command: "download_model", args: { model: "whisperSmall" } },
    ]);
  });

  it("shows the Record Shortcut on Try it and finishes into the main settings", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    expect(await screen.findByRole("heading", { level: 1, name: "Try it" })).toBeVisible();
    // The sample phrase is now the test field's own placeholder rather than a separate line, and
    // the instruction leads into it (rule 2.4).
    expect(screen.getByText("Ctrl + Space").closest("p")).toHaveTextContent(
      "Hold Ctrl + Space and say:",
    );
    const field = screen.getByRole("textbox", { name: "Test field" });
    expect(field).toBeVisible();
    expect(field).toHaveAttribute("placeholder", "Hello Echo, can you hear me?");
    expect(field).toHaveAccessibleDescription("Try it Hold Ctrl + Space and say:");

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
  // Rule 2.1: the Welcome line follows the Record Shortcut mode.
  it("describes holding the shortcut in Push-to-Talk Mode and pressing it twice in Toggle Mode", async () => {
    const { unmount } = render(<App />);
    expect(
      await screen.findByText(
        "Then hold a shortcut, speak, and let go. The text appears where you are typing.",
      ),
    ).toBeVisible();
    unmount();

    backend.settings.shortcutMode = "toggle";
    render(<App />);
    expect(
      await screen.findByText(
        "Then press a shortcut, speak, and press it again. The text appears where you are typing.",
      ),
    ).toBeVisible();
  });

  // Acceptance test 13 and models.md rule 30.
  it("says a graphics card was found and marks Whisper large-v3-turbo Recommended", async () => {
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    expect(
      await screen.findByText("Graphics card found — recommended: Whisper large-v3-turbo"),
    ).toBeVisible();
    expect(
      within(screen.getByRole("listitem", { name: "Whisper large-v3-turbo" })).getByText(
        "Recommended",
      ),
    ).toBeVisible();
  });

  it("says no graphics card was found and marks Parakeet TDT 0.6B v3 Recommended", async () => {
    backend.computeHardware = "cpu";
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    expect(
      await screen.findByText("No graphics card found — Parakeet TDT 0.6B v3 is faster on this PC"),
    ).toBeVisible();
    expect(screen.getByRole("listitem", { name: "Parakeet TDT 0.6B v3" })).toHaveTextContent(
      "Recommended",
    );
    expect(screen.getByRole("listitem", { name: "Whisper large-v3-turbo" })).not.toHaveTextContent(
      "Recommended",
    );
    await userEvent.click(screen.getByRole("button", { name: "Download (705 MB)" }));
    expect(backend.commandsCalled("download_model")).toEqual([
      { command: "download_model", args: { model: "parakeetTdt06bV3" } },
    ]);
  });

  // Acceptance test 14 and rule 2a.
  it("goes Back from Choose a Model to Welcome and forward again", async () => {
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Back" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Welcome to Echo" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Back" })).not.toBeInTheDocument();
    expect(backend.settings.onboardingWelcomeDone).toBe(true);

    await userEvent.click(screen.getByRole("button", { name: "Get started" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
  });

  it("goes Back from Try it to Choose a Model, which continues to Try it", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    await screen.findByRole("heading", { level: 1, name: "Try it" });
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Choose a Model" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Finish later" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Try it" })).toBeVisible();
  });

  // Acceptance test 15 and rule 3a.
  it("finishes later without a Model and lands on the Model page with its start panel", async () => {
    backend.settings.onboardingWelcomeDone = true;
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Finish later" }));

    expect(await screen.findByRole("navigation", { name: "Sections" })).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(true);
    expect(screen.getByRole("heading", { name: "Download a Model to start" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Download (845 MB)" })).toBeVisible();
    expect(backend.commandsCalled("activate_model")).toHaveLength(0);
  });

  // Acceptance test 16, rule 2.4.
  it("acknowledges text arriving in the test field and opens the main window by itself", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    const field = await screen.findByRole("textbox", { name: "Test field" });
    expect(screen.queryByText("Echo heard you.")).not.toBeInTheDocument();
    fireEvent.input(field, { target: { value: "Hello from Echo" } });

    expect(await screen.findByText("Echo heard you.")).toBeVisible();
    expect(screen.getByText("Opening Echo…")).toBeVisible();
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    expect(backend.settings.onboardingCompleted).toBe(false);

    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(await screen.findByRole("navigation", { name: "Sections" })).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(true);
  });

  // Rule 2.4: during onboarding the Transcript comes to the field from the backend, so it lands
  // there even when keyboard focus is elsewhere.
  it("puts a Transcript sent for Try it into the test field wherever focus is", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    const field = await screen.findByRole("textbox", { name: "Test field" });
    field.blur();
    act(() => {
      backend.emit("try-it-transcript", "Hello Echo, can you hear me?");
    });

    expect(await screen.findByText("Echo heard you.")).toBeVisible();
    expect(field).toHaveValue("Hello Echo, can you hear me?");
  });

  it("focuses the test field on Try it so a Dictation has somewhere to land", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    expect(await screen.findByRole("textbox", { name: "Test field" })).toHaveFocus();
  });

  // Rule 2.4: the status line follows the Dictation with the Overlay's words.
  it("shows the Dictation's phase under the test field", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);
    await screen.findByRole("textbox", { name: "Test field" });

    act(() => {
      backend.changeDictation({ state: "recording", listening: false });
    });
    expect(await screen.findByText("Getting ready…")).toBeVisible();

    act(() => {
      backend.changeDictation({ state: "recording", listening: true });
    });
    expect(await screen.findByText("Listening")).toBeVisible();
    expect(screen.queryByText("Getting ready…")).not.toBeInTheDocument();

    act(() => {
      backend.changeDictation({ state: "transcribing", listening: false });
    });
    expect(await screen.findByText("Transcribing…")).toBeVisible();

    act(() => {
      backend.changeDictation({ state: "idle" });
    });
    expect(screen.queryByText("Transcribing…")).not.toBeInTheDocument();
  });

  it("stays on Try it when a new Dictation starts before it moves on", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    const field = await screen.findByRole("textbox", { name: "Test field" });
    fireEvent.input(field, { target: { value: "Hello Echo" } });
    expect(await screen.findByText("Opening Echo…")).toBeVisible();
    act(() => {
      backend.changeDictation({ state: "recording", listening: true });
    });
    expect(await screen.findByText("Listening")).toBeVisible();
    expect(screen.queryByText("Opening Echo…")).not.toBeInTheDocument();
    act(() => {
      backend.changeDictation({ state: "idle", listening: false });
      vi.advanceTimersByTime(4000);
    });

    expect(screen.getByRole("heading", { level: 1, name: "Try it" })).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Finish" }));
    expect(await screen.findByRole("navigation", { name: "Sections" })).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(true);
  });

  it("stays on Try it when the user types in the test field before it moves on", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    const field = await screen.findByRole("textbox", { name: "Test field" });
    fireEvent.input(field, { target: { value: "Hello Echo" } });
    expect(await screen.findByText("Opening Echo…")).toBeVisible();
    fireEvent.keyDown(field, { key: "!" });
    expect(screen.queryByText("Opening Echo…")).not.toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(4000);
    });

    expect(screen.getByRole("heading", { level: 1, name: "Try it" })).toBeVisible();
    expect(screen.getByText("Echo heard you.")).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(false);
  });

  it("shows Try it in Polish", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.settings.uiLanguage = "pl";
    backend.models.active = "whisperSmall";
    await changeUiLanguage("pl");
    render(<App />);

    const field = await screen.findByRole("textbox", { name: "Pole testowe" });
    expect(field).toHaveAttribute("placeholder", "Cześć Echo, słyszysz mnie?");
    expect(screen.getByText("Echo jest gotowe. Powiedz poniższe zdanie.")).toBeVisible();
    expect(screen.getByText(/^Przytrzymaj/)).toHaveTextContent(/^Przytrzymaj .+ i powiedz:$/);
    act(() => {
      backend.changeDictation({ state: "recording", listening: true });
    });
    expect(await screen.findByText("Słucham")).toBeVisible();
    act(() => {
      backend.changeDictation({ state: "idle", listening: false });
    });
    fireEvent.input(field, { target: { value: "Cześć" } });
    expect(await screen.findByText("Echo cię słyszy.")).toBeVisible();
    expect(screen.getByText("Otwieram Echo…")).toBeVisible();
  });

  it("lists what to check when nothing appeared, with actions", async () => {
    backend.settings.onboardingWelcomeDone = true;
    backend.models.active = "whisperSmall";
    render(<App />);

    await userEvent.click(await screen.findByText("Nothing appeared?"));

    await userEvent.click(screen.getByRole("button", { name: "Windows privacy settings" }));
    expect(backend.commandsCalled("open_microphone_privacy_settings")).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: "Open log folder" }));
    expect(backend.commandsCalled("open_log_folder")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Model settings" })).toBeVisible();

    await userEvent.click(screen.getByRole("button", { name: "Microphone settings" }));
    expect(await screen.findByRole("navigation", { name: "Sections" })).toBeVisible();
    expect(backend.settings.onboardingCompleted).toBe(true);
    expect(screen.getByRole("heading", { level: 1, name: "Dictation" })).toBeVisible();
  });

  // Acceptance test 17, rule 2b.
  it("moves focus to the new step's heading and marks completed steps with a check", async () => {
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Get started" }));

    const heading = await screen.findByRole("heading", { level: 1, name: "Choose a Model" });
    expect(heading).toHaveFocus();
    const progress = screen.getByRole("list", { name: "Setup steps" });
    const [welcome, model, tryIt] = within(progress).getAllByRole("listitem");
    expect(welcome).toHaveTextContent("Welcome, done");
    expect(welcome?.querySelector("svg")).not.toBeNull();
    expect(model).toHaveAttribute("aria-current", "step");
    expect(model).not.toHaveTextContent("done");
    expect(tryIt?.querySelector("svg")).toBeNull();
  });
});
