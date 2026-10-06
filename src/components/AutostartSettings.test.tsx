import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState({ ...initialShell, page: "app" }, true);
  await changeUiLanguage("en");
});

function toggle() {
  return screen.findByRole("switch", { name: "Start Echo when I sign in to Windows" });
}

describe("Start with Windows", () => {
  // autostart.md rule 1.
  it("is off by default, with its one-line description", async () => {
    render(<App />);

    expect(await toggle()).not.toBeChecked();
    expect(screen.getByText("Echo starts in the tray when you sign in.")).toBeVisible();
  });

  // autostart.md rules 2–3: the change is stored at once and applied by the backend.
  it("turns on and off through the setting", async () => {
    render(<App />);

    await userEvent.click(await toggle());
    await waitFor(() => {
      expect(backend.settings.startWithWindows).toBe(true);
    });
    expect(await toggle()).toBeChecked();

    await userEvent.click(await toggle());
    await waitFor(() => {
      expect(backend.settings.startWithWindows).toBe(false);
    });
    expect(await toggle()).not.toBeChecked();
  });

  // autostart.md acceptance test 6 (UI half).
  it("shows off with a hint and a link when Windows has turned Echo off", async () => {
    backend.settings = { ...backend.settings, startWithWindows: true };
    backend.autostartDisabledInWindows = true;
    render(<App />);

    const control = await toggle();
    await waitFor(() => {
      expect(control).not.toBeChecked();
    });
    expect(control).toBeDisabled();
    expect(screen.getByText("Turned off in Windows Startup apps")).toBeVisible();

    await userEvent.click(screen.getByRole("button", { name: "Open Startup apps" }));
    expect(backend.commandsCalled("open_startup_apps_settings")).toHaveLength(1);
  });

  it("is labelled in Polish", async () => {
    backend.settings = { ...backend.settings, uiLanguage: "pl" };
    render(<App />);

    expect(
      await screen.findByRole("switch", { name: "Uruchamiaj Echo po zalogowaniu do Windows" }),
    ).toBeVisible();
  });
});
