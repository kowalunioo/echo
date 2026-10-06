import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "./App";
import { changeUiLanguage } from "./i18n";
import { useShell } from "./store/shell";
import { backend } from "./test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState(initialShell, true);
  await changeUiLanguage("en");
});

describe("app shell", () => {
  it("shows nothing until the settings have loaded", () => {
    backend.hanging.add("get_settings");
    const { container } = render(<App />);

    expect(container).toBeEmptyDOMElement();
  });

  it("shows the five sections in the navigation", async () => {
    render(<App />);

    const nav = await screen.findByRole("navigation", { name: "Sections" });
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
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "History" }));

    expect(screen.getByRole("heading", { level: 1, name: "History" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "History" })).toHaveAttribute("aria-current", "page");
  });

  it("uses the saved UI Language", async () => {
    backend.settings.uiLanguage = "pl";
    render(<App />);

    expect(await screen.findByRole("navigation", { name: "Sekcje" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Historia" })).toBeInTheDocument();
  });

  it("shows the version from the app_info command on the App page", async () => {
    backend.appInfo = { version: "9.8.7", systemLocale: "pl-PL" };
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    await waitFor(() => {
      expect(screen.getByTestId("app-version")).toHaveTextContent("9.8.7");
    });
  });

  // settings-and-first-run.md rules 8 and 11.
  it("switches the UI Language immediately and saves it", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    await userEvent.click(screen.getByRole("radio", { name: "Polski" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Aplikacja" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Polski" })).toBeChecked();
    expect(document.documentElement.lang).toBe("pl");
    expect(backend.settings.uiLanguage).toBe("pl");
  });

  // Acceptance test 7 (window part): a change made elsewhere, e.g. from the tray, applies live.
  it("follows a UI Language change made outside the window", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    backend.changeSettings({ uiLanguage: "pl" });

    expect(await screen.findByRole("navigation", { name: "Sekcje" })).toBeInTheDocument();
  });

  it("opens the log folder from the App page", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    await userEvent.click(screen.getByRole("button", { name: "Open log folder" }));

    expect(backend.commandsCalled("open_log_folder")).toHaveLength(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says so when the log folder cannot be opened", async () => {
    backend.failing.set("open_log_folder", "no explorer");
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    await userEvent.click(screen.getByRole("button", { name: "Open log folder" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The log folder could not be opened.",
    );
  });

  it("explains when the settings cannot be loaded", async () => {
    backend.failing.set("get_settings", "broken backend");
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("could not load its settings");
  });
});
