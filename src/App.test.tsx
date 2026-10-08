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
  it("shows only the title bar until the settings have loaded", () => {
    backend.hanging.add("get_settings");
    render(<App />);

    // The window has no native frame, so its own bar must be there to move or close it.
    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close window" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
    expect(screen.queryByRole("main")).not.toBeInTheDocument();
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

  it("switches pages with Ctrl+1 to Ctrl+5 and names the keys on each section", async () => {
    render(<App />);
    const nav = await screen.findByRole("navigation", { name: "Sections" });
    expect(
      within(nav)
        .getAllByRole("button")
        .map((b) => b.getAttribute("aria-keyshortcuts")),
    ).toEqual(["Control+1", "Control+2", "Control+3", "Control+4", "Control+5"]);

    await userEvent.keyboard("{Control>}4{/Control}");
    expect(await screen.findByRole("heading", { level: 1, name: "History" })).toBeVisible();
    await userEvent.keyboard("{Control>}5{/Control}");
    expect(await screen.findByRole("heading", { level: 1, name: "App" })).toBeVisible();
    await userEvent.keyboard("{Control>}1{/Control}");
    expect(await screen.findByRole("heading", { level: 1, name: "Dictation" })).toBeVisible();
  });

  it("leaves Ctrl+digit alone while typing in a field", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });
    await userEvent.keyboard("{Control>}3{/Control}");
    const input = await screen.findByRole("textbox", { name: "Add a word or phrase" });
    await userEvent.click(input);
    await userEvent.keyboard("{Control>}1{/Control}");
    expect(screen.getByRole("heading", { level: 1, name: "Vocabulary" })).toBeVisible();
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

  it("names the Windows display language in the UI Language, with the code as a tooltip", async () => {
    backend.appInfo = { version: "9.8.7", systemLocale: "pl-PL" };
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    const language = await screen.findByText("Polish (Poland)");
    expect(language).toHaveAttribute("title", "pl-PL");
  });

  it("shows the raw Windows locale when it has no language name", async () => {
    backend.appInfo = { version: "9.8.7", systemLocale: "x-unknown" };
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    expect(await screen.findByText("x-unknown")).toBeVisible();
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

  it("says on the App page that dictation stays on this computer", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "App" }));

    expect(
      screen.getByText(
        "Your voice and Transcripts stay on this computer. Echo goes online only to download Models and check for updates.",
      ),
    ).toBeVisible();
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

  // Windows does not run Echo's keyboard hook while Echo has focus, so the window forwards keys.
  it("forwards keys pressed in Echo's window to the backend, in order", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    await userEvent.keyboard("{Control>}{ }{/Control}");

    await waitFor(() => {
      expect(backend.commandsCalled("own_window_key").map((call) => call.args)).toEqual([
        { key: "LeftCtrl", pressed: true },
        { key: "Space", pressed: true },
        { key: "Space", pressed: false },
        { key: "LeftCtrl", pressed: false },
      ]);
    });
  });
});
