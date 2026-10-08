import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "../App";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState(initialShell, true);
  backend.microphones = { devices: ["Laptop Mic", "USB Mic"], default: "Laptop Mic" };
  await changeUiLanguage("en");
});

async function picker() {
  return screen.findByRole("button", { name: /^Microphone:/ });
}

async function openPicker() {
  await userEvent.click(await picker());
  return screen.findByRole("listbox", { name: "Microphone" });
}

describe("Microphone picker", () => {
  it("sits under its own Microphone heading", async () => {
    render(<App />);
    expect(
      await screen.findByRole("heading", { level: 2, name: "Microphone" }),
    ).toBeInTheDocument();
  });

  it("shows Default with the Windows default device's name", async () => {
    render(<App />);

    expect(
      await screen.findByRole("button", { name: "Microphone: Default (Laptop Mic)" }),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Use default" })).not.toBeInTheDocument();
  });

  it("lists Default first, then every input device", async () => {
    render(<App />);

    const list = await openPicker();
    await waitFor(() => {
      expect(
        within(list)
          .getAllByRole("option")
          .map((o) => o.textContent),
      ).toEqual(["Default (Laptop Mic)", "Laptop Mic", "USB Mic"]);
    });
    expect(within(list).getByRole("option", { name: "Default (Laptop Mic)" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  // microphone.md acceptance test 7.
  it("requests a fresh device list every time the picker opens", async () => {
    render(<App />);
    const button = await picker();
    await waitFor(() => {
      expect(backend.commandsCalled("list_microphones")).toHaveLength(1);
    });

    await userEvent.click(button);
    await waitFor(() => {
      expect(backend.commandsCalled("list_microphones")).toHaveLength(2);
    });
    await userEvent.click(button);
    backend.microphones = {
      devices: ["Laptop Mic", "USB Mic", "Headset"],
      default: "Laptop Mic",
    };
    await userEvent.click(button);

    await waitFor(() => {
      expect(backend.commandsCalled("list_microphones")).toHaveLength(3);
    });
    expect(
      await within(screen.getByRole("listbox")).findByRole("option", { name: "Headset" }),
    ).toBeInTheDocument();
  });

  it("saves the chosen device at once", async () => {
    render(<App />);

    const list = await openPicker();
    await userEvent.click(await within(list).findByRole("option", { name: "USB Mic" }));

    await waitFor(() => {
      expect(backend.settings.microphone).toEqual({ kind: "device", name: "USB Mic" });
    });
    expect(screen.getByRole("button", { name: "Microphone: USB Mic" })).toHaveFocus();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("can be used with the keyboard", async () => {
    render(<App />);
    const button = await picker();
    await waitFor(() => {
      expect(backend.commandsCalled("list_microphones")).toHaveLength(1);
    });

    button.focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(await screen.findByRole("listbox")).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");

    await waitFor(() => {
      expect(backend.settings.microphone).toEqual({ kind: "device", name: "USB Mic" });
    });

    await userEvent.keyboard("{ArrowDown}");
    await screen.findByRole("listbox");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(backend.settings.microphone).toEqual({ kind: "device", name: "USB Mic" });
  });

  it("marks a stored device that is missing as not connected and keeps it selected", async () => {
    backend.settings = { ...backend.settings, microphone: { kind: "device", name: "Old Mic" } };
    render(<App />);

    expect(
      await screen.findByRole("button", { name: "Microphone: Old Mic (not connected)" }),
    ).toBeVisible();
    const list = await openPicker();
    expect(within(list).getByRole("option", { name: "Old Mic (not connected)" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(backend.settings.microphone).toEqual({ kind: "device", name: "Old Mic" });
  });

  it("resets to Default", async () => {
    backend.settings = { ...backend.settings, microphone: { kind: "device", name: "USB Mic" } };
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "Use default" }));

    await waitFor(() => {
      expect(backend.settings.microphone).toEqual({ kind: "default" });
    });
    expect(
      await screen.findByRole("button", { name: "Microphone: Default (Laptop Mic)" }),
    ).toBeVisible();
  });

  it("says so when there is no microphone at all", async () => {
    backend.microphones = { devices: [], default: null };
    render(<App />);

    expect(await screen.findByText("No microphone found. Connect one to dictate.")).toBeVisible();
    expect(screen.getByRole("button", { name: "Microphone: Default" })).toBeVisible();
  });

  it("still offers Default when the device list cannot be loaded", async () => {
    backend.failing.set("list_microphones", "boom");
    render(<App />);

    const list = await openPicker();
    expect(
      await within(list).findByText("The list of microphones could not be loaded."),
    ).toBeVisible();
    expect(within(list).getByRole("option", { name: "Default" })).toBeInTheDocument();
  });

  it("is translated into Polish", async () => {
    backend.settings = { ...backend.settings, uiLanguage: "pl" };
    render(<App />);

    expect(
      await screen.findByRole("button", { name: "Mikrofon: Domyślny (Laptop Mic)" }),
    ).toBeVisible();
  });
});
