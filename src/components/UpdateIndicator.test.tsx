import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState(initialShell, true);
  await changeUiLanguage("en");
});

async function status() {
  render(<App />);
  const region = await screen.findByRole("region", { name: "Status" });
  await within(region).findByTestId("status-version");
  return within(region);
}

/** The line's one button: "Check for updates", or the updater's status while there is one. */
function link(region: Awaited<ReturnType<typeof status>>) {
  return region.getAllByRole("button").at(-1) as HTMLElement;
}

// updater.md "UI": the version and a manual check are the sidebar's last line.
describe("Updates line in the sidebar", () => {
  it("shows the running version and checks for updates on click", async () => {
    const region = await status();

    expect(region.getByTestId("status-version")).toHaveTextContent("Echo 0.1.0");
    const button = region.getByRole("button", { name: "Check for updates" });

    await userEvent.click(button);
    expect(backend.commandsCalled("check_for_updates")).toHaveLength(1);
  });

  it("shows the updater's status in the link's place", async () => {
    const region = await status();

    act(() => {
      backend.changeUpdater({ status: { state: "checking" } });
    });
    expect(link(region)).toHaveTextContent("Checking…");
    expect(region.getByTestId("status-version")).toBeVisible();

    act(() => {
      backend.changeUpdater({ status: { state: "upToDate" } });
    });
    expect(link(region)).toHaveTextContent("Echo is up to date");

    // A failure opens the App page on click, which explains it.
    act(() => {
      backend.changeUpdater({ status: { state: "checkFailed" } });
    });
    expect(link(region)).toHaveTextContent("Couldn't check for updates");
    expect(region.getByTestId("status-version")).toHaveTextContent("Echo 0.1.0");
    await userEvent.click(link(region));
    expect(backend.commandsCalled("check_for_updates")).toHaveLength(0);
    expect(await screen.findByRole("heading", { level: 1, name: "App" })).toBeVisible();
  });

  it("offers to install and restart once a newer version is there", async () => {
    const region = await status();

    act(() => {
      backend.changeUpdater({ status: { state: "available", version: "0.2.0" } });
    });
    const button = region.getByRole("button", { name: "Install and restart" });
    expect(button).toHaveAttribute("title", "Version 0.2.0 is available");

    await userEvent.click(button);
    expect(backend.commandsCalled("install_update")).toHaveLength(1);

    // An automatic update that is downloaded installs by itself when Echo is Idle; the button
    // lets the user go first.
    act(() => {
      backend.changeUpdater({ status: { state: "ready", version: "0.2.0" } });
    });
    expect(region.getByRole("button", { name: "Install and restart" })).toBeVisible();
  });

  it("shows only the version when updates are managed by the system", async () => {
    backend.changeUpdater({ managed: true });
    const region = await status();

    expect(region.getByTestId("status-version")).toHaveTextContent("Echo 0.1.0");
    expect(region.queryByRole("button", { name: "Check for updates" })).toBeNull();
  });
});
