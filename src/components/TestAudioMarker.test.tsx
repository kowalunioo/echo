import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { backend } from "../test/backend";
import { TestAudioMarker } from "./TestAudioMarker";

const setTitle = vi.fn<(title: string) => Promise<void>>(() => Promise.resolve());
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ setTitle }),
}));

describe("test-audio marker (dictation-pipeline.md rules 40–41)", () => {
  beforeEach(async () => {
    setTitle.mockClear();
    await changeUiLanguage("en");
  });

  it("shows nothing and keeps the window title in normal operation", async () => {
    backend.settings = { ...backend.settings, onboardingCompleted: true };
    render(<App />);
    await screen.findByRole("navigation");
    await waitFor(() => {
      expect(backend.calls.some((c) => c.command === "get_test_audio")).toBe(true);
    });
    expect(screen.queryByRole("status", { name: /test audio/i })).not.toBeInTheDocument();
    expect(setTitle).not.toHaveBeenCalled();
  });

  it("marks the main window and its title with the WAV file", async () => {
    backend.testAudio = "pl-proste.wav";
    backend.settings = { ...backend.settings, onboardingCompleted: true };
    render(<App />);
    const marker = await screen.findByRole("status", { name: "Test audio" });
    expect(marker).toHaveTextContent("pl-proste.wav");
    expect(marker).toHaveTextContent(/instead of the microphone/i);
    await waitFor(() => {
      expect(setTitle).toHaveBeenLastCalledWith("Echo — TEST AUDIO: pl-proste.wav");
    });
  });

  it("is shown during onboarding too", async () => {
    backend.testAudio = "cisza.wav";
    backend.settings = { ...backend.settings, onboardingCompleted: false };
    render(<App />);
    expect(await screen.findByRole("status", { name: "Test audio" })).toHaveTextContent(
      "cisza.wav",
    );
  });

  it("follows the UI Language", async () => {
    backend.testAudio = "pl-proste.wav";
    await changeUiLanguage("pl");
    render(<TestAudioMarker />);
    const marker = await screen.findByRole("status", { name: "Dźwięk testowy" });
    expect(marker).toHaveTextContent("pl-proste.wav");
    await waitFor(() => {
      expect(setTitle).toHaveBeenLastCalledWith("Echo — DŹWIĘK TESTOWY: pl-proste.wav");
    });
  });

  it("has a compact form for the Overlay that leaves the window title alone", async () => {
    backend.testAudio = "pl-proste.wav";
    render(<TestAudioMarker compact />);
    const marker = await screen.findByRole("status", { name: "Test audio" });
    expect(marker).toHaveTextContent("TEST AUDIO");
    expect(setTitle).not.toHaveBeenCalled();
  });
});
