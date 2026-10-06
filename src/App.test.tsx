import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import type { AppInfo } from "./bindings";
import { changeUiLanguage, initI18n } from "./i18n";
import { useShell } from "./store/shell";

// The generated bindings call Tauri's `invoke`; faking it exercises the real binding code.
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => undefined),
}));

const polishWindows: AppInfo = {
  version: "0.1.0",
  systemLocale: "pl-PL",
  defaultUiLanguage: "pl",
};

const initialState = useShell.getState();
initI18n("en");

beforeEach(async () => {
  invoke.mockReset();
  useShell.setState(initialState, true);
  await changeUiLanguage("en");
});

function respondWith(info: AppInfo) {
  invoke.mockImplementation((command: string) => {
    if (command === "app_info") return Promise.resolve(info);
    // The Dictation page loads the Record Shortcut; its own tests cover that.
    if (command === "record_shortcut") return new Promise(() => undefined);
    if (command === "own_window_key") return Promise.resolve(null);
    return Promise.reject(new Error(`unexpected ${command}`));
  });
}

describe("app shell", () => {
  it("shows the five sections in the navigation", () => {
    invoke.mockReturnValue(new Promise(() => undefined));
    render(<App />);

    const nav = screen.getByRole("navigation", { name: "Sections" });
    const labels = within(nav)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(labels).toEqual(["Dictation", "Model & language", "Vocabulary", "History", "App"]);
    expect(within(nav).getByRole("button", { name: "Dictation" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("switches pages from the navigation", async () => {
    invoke.mockReturnValue(new Promise(() => undefined));
    render(<App />);

    await userEvent.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByRole("heading", { level: 1, name: "History" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "History" })).toHaveAttribute("aria-current", "page");
  });

  it("follows the Windows display language reported by the backend", async () => {
    respondWith(polishWindows);
    render(<App />);

    expect(await screen.findByRole("navigation", { name: "Sekcje" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Historia" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("app_info");
  });

  it("shows the version from the app_info command on the App page", async () => {
    respondWith({ ...polishWindows, defaultUiLanguage: "en", version: "9.8.7" });
    render(<App />);

    await userEvent.click(screen.getByRole("button", { name: "App" }));

    await waitFor(() => {
      expect(screen.getByTestId("app-version")).toHaveTextContent("9.8.7");
    });
  });

  it("switches the UI Language immediately and keeps the user's choice", async () => {
    respondWith({ ...polishWindows, defaultUiLanguage: "en" });
    render(<App />);
    await userEvent.click(screen.getByRole("button", { name: "App" }));

    await userEvent.click(screen.getByRole("radio", { name: "Polski" }));

    expect(screen.getByRole("heading", { level: 1, name: "Aplikacja" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Polski" })).toBeChecked();
    expect(document.documentElement.lang).toBe("pl");
  });

  // Windows does not run Echo's keyboard hook while Echo has focus, so the window forwards keys.
  it("forwards keys pressed in Echo's window to the backend", async () => {
    respondWith(polishWindows);
    render(<App />);

    await userEvent.keyboard("{Control>}{ }{/Control}");

    // In order, each sent after the previous one was handled.
    await waitFor(() => {
      expect(
        invoke.mock.calls
          .filter(([command]) => command === "own_window_key")
          .map(([, a]) => a as unknown),
      ).toEqual([
        { key: "LeftCtrl", pressed: true },
        { key: "Space", pressed: true },
        { key: "Space", pressed: false },
        { key: "LeftCtrl", pressed: false },
      ]);
    });
  });
});
