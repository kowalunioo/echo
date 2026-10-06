import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";
import { UPDATED_NOTICE_MS } from "./UpdatedNotice";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState({ ...initialShell, page: "app" }, true);
  await changeUiLanguage("en");
});

async function status() {
  return screen.findByTestId("update-status");
}

describe("updates on the App page (updater.md UI)", () => {
  it("turns automatic checks off and on through the setting (rule 10)", async () => {
    render(<App />);
    const toggle = await screen.findByRole("switch", {
      name: "Check for updates automatically",
    });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    await userEvent.click(toggle);
    await waitFor(() => {
      expect(backend.settings.checkUpdatesAutomatically).toBe(false);
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    // Manual checks remain available.
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeEnabled();
  });

  it("runs a manual check and shows its progress and result (rule 7)", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    expect(backend.commandsCalled("check_for_updates")).toHaveLength(1);

    act(() => {
      backend.changeUpdater({ status: { state: "checking" } });
    });
    expect(await status()).toHaveTextContent("Checking…");
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeDisabled();

    act(() => {
      backend.changeUpdater({ status: { state: "upToDate" } });
    });
    expect(await status()).toHaveTextContent("Echo is up to date");

    act(() => {
      backend.changeUpdater({ status: { state: "idle" } });
    });
    await waitFor(() => {
      expect(screen.queryByTestId("update-status")).not.toBeInTheDocument();
    });
  });

  it("installs an available version only when the user confirms (rules 7–8)", async () => {
    backend.updater = { ...backend.updater, status: { state: "available", version: "0.2.0" } };
    render(<App />);
    expect(await status()).toHaveTextContent("Version 0.2.0 is available");
    expect(backend.commandsCalled("install_update")).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(backend.commandsCalled("install_update")).toHaveLength(1);

    act(() => {
      backend.changeUpdater({ status: { state: "downloading", version: "0.2.0", percent: 42 } });
    });
    expect(await status()).toHaveTextContent("Downloading… 42%");
    act(() => {
      backend.changeUpdater({ status: { state: "installing", version: "0.2.0" } });
    });
    expect(await status()).toHaveTextContent("Installing…");
  });

  it.each([
    ["checkFailed", "Couldn't check for updates"],
    ["unverified", "Update could not be verified"],
    ["downloadFailed", "Couldn't download the update"],
  ] as const)("shows the %s failure", async (state, text) => {
    backend.updater = { ...backend.updater, status: { state } };
    render(<App />);
    expect(await status()).toHaveTextContent(text);
  });

  it("is read-only with an explanation when updates are managed by the system (rule 11)", async () => {
    backend.updater = { ...backend.updater, managed: true };
    render(<App />);
    const toggle = await screen.findByRole("switch", {
      name: "Check for updates automatically",
    });
    await screen.findByText("Updates are managed by your system.");
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(screen.queryByRole("button", { name: "Check for updates" })).not.toBeInTheDocument();
  });

  it("speaks Polish", async () => {
    backend.settings = { ...backend.settings, uiLanguage: "pl" };
    backend.updater = { ...backend.updater, status: { state: "available", version: "0.2.0" } };
    render(<App />);
    await screen.findByRole("switch", { name: "Automatycznie sprawdzaj aktualizacje" });
    expect(await status()).toHaveTextContent("Dostępna jest wersja 0.2.0");
    expect(screen.getByRole("button", { name: "Zainstaluj i uruchom ponownie" })).toBeVisible();
  });
});

describe("the updated notice (rule 6)", () => {
  it("announces the new version and can be dismissed", async () => {
    backend.updater = { ...backend.updater, updatedTo: "0.2.0" };
    render(<App />);
    expect(await screen.findByText("Echo was updated to 0.2.0")).toBeVisible();

    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => {
      expect(screen.queryByText("Echo was updated to 0.2.0")).not.toBeInTheDocument();
    });
    expect(backend.commandsCalled("dismiss_update_notice")).toHaveLength(1);
  });

  it("goes away on its own shortly after it is seen", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      backend.updater = { ...backend.updater, updatedTo: "0.2.0" };
      render(<App />);
      await screen.findByText("Echo was updated to 0.2.0");
      await act(async () => {
        await vi.advanceTimersByTimeAsync(UPDATED_NOTICE_MS);
      });
      expect(backend.commandsCalled("dismiss_update_notice")).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
