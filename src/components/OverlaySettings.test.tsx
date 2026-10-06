import { act, render, screen, waitFor } from "@testing-library/react";
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

describe("Overlay settings", () => {
  // overlay.md "Settings" and rule 17.
  it("shows the recording indicator by default and turns it off", async () => {
    render(<App />);
    const toggle = await screen.findByRole("switch", { name: "Show recording indicator" });
    expect(toggle).toBeChecked();

    await userEvent.click(toggle);
    await waitFor(() => {
      expect(backend.settings.showOverlay).toBe(false);
    });
    expect(toggle).not.toBeChecked();
  });

  // Rules 14 and 16.
  it("moves the indicator between bottom and top", async () => {
    render(<App />);
    const position = await screen.findByRole("group", { name: "Position" });
    expect(screen.getByRole("radio", { name: "Bottom" })).toBeChecked();
    expect(position).toBeVisible();

    await userEvent.click(screen.getByRole("radio", { name: "Top" }));
    await waitFor(() => {
      expect(backend.settings.overlayPosition).toBe("top");
    });
    expect(screen.getByRole("radio", { name: "Top" })).toBeChecked();
  });

  it("is labelled in Polish too", async () => {
    backend.settings.uiLanguage = "pl";
    render(<App />);
    expect(
      await screen.findByRole("switch", { name: "Pokazuj wskaźnik nagrywania" }),
    ).toBeVisible();
    expect(screen.getByRole("radio", { name: "Na dole" })).toBeChecked();
  });
});

describe("Opening a page from the Overlay", () => {
  // overlay.md rule 5: the "No Model" message opens the Models page.
  it("switches the main window to the requested page", async () => {
    render(<App />);
    await screen.findByRole("switch", { name: "Show recording indicator" });
    act(() => {
      backend.emit("main-page-requested", "model");
    });
    expect(await screen.findByRole("heading", { name: "Model & language" })).toBeVisible();
  });
});
