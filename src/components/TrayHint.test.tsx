import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { backend } from "../test/backend";

beforeEach(async () => {
  await changeUiLanguage("en");
});

async function requestHint() {
  await screen.findByRole("heading", { level: 1 });
  act(() => {
    backend.emit("tray-hint-requested", null);
  });
  return screen.findByRole("dialog");
}

describe("Close-to-tray hint", () => {
  it("is hidden until the backend asks for it", async () => {
    render(<App />);
    await screen.findByRole("heading", { level: 1 });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("explains that Echo keeps running and hides to the tray when confirmed", async () => {
    render(<App />);
    const dialog = await requestHint();
    expect(dialog).toHaveAccessibleName("Echo keeps running");
    expect(dialog).toHaveTextContent(
      "Echo is still running in the tray. Use Quit Echo from the tray menu to exit.",
    );

    await userEvent.click(screen.getByRole("button", { name: "OK" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.commandsCalled("close_to_tray")).toHaveLength(1);
    expect(backend.settings.trayHintShown).toBe(true);
  });

  it("is dismissed with Escape too", async () => {
    render(<App />);
    await requestHint();

    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.commandsCalled("close_to_tray")).toHaveLength(1);
  });

  it("is in Polish when the UI Language is Polish", async () => {
    backend.settings = { ...backend.settings, uiLanguage: "pl" };
    render(<App />);
    const dialog = await requestHint();
    expect(dialog).toHaveTextContent("Zakończ Echo");
    expect(screen.getByRole("button", { name: "OK" })).toHaveFocus();
  });
});
