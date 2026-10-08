import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "../App";
import type { ModelId } from "../bindings";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend } from "../test/backend";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState(initialShell, true);
  await changeUiLanguage("en");
});

async function openModelsPage() {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Model & language" }));
  return screen.findByRole("region", { name: "Models" });
}

function card(name: string) {
  return screen.getByRole("listitem", { name });
}

function downloaded(...ids: ModelId[]) {
  for (const id of ids) {
    const entry = backend.models.models.find((m) => m.id === id);
    if (entry) entry.downloaded = true;
  }
}

describe("Models page", () => {
  it("lists the three Models in order with size, languages and the Recommended badge", async () => {
    const region = await openModelsPage();

    const items = within(region).getAllByRole("listitem");
    expect(items.map((li) => li.getAttribute("aria-label"))).toEqual([
      "Whisper large-v3-turbo",
      "Parakeet TDT 0.6B v3",
      "Whisper small",
    ]);
    const turbo = card("Whisper large-v3-turbo");
    expect(turbo).toHaveTextContent("Recommended");
    expect(turbo).toHaveTextContent("845 MB · 100 languages");
    expect(card("Parakeet TDT 0.6B v3")).toHaveTextContent("705 MB · 25 European languages");
    expect(card("Whisper small")).not.toHaveTextContent("Recommended");
    expect(
      within(card("Whisper small")).getByRole("button", { name: "Download (257 MB)" }),
    ).toBeVisible();
  });

  it("starts a download and shows its progress, speed and a Cancel named for its Model", async () => {
    await openModelsPage();

    await userEvent.click(within(card("Whisper small")).getByRole("button", { name: /Download/ }));
    expect(backend.commandsCalled("download_model")[0]?.args).toEqual({ model: "whisperSmall" });

    act(() => {
      backend.changeModel("whisperSmall", {
        download: {
          state: "downloading",
          downloaded: 80_925_341,
          total: 269_751_136,
          bytesPerSecond: 5_452_595,
        },
      });
      backend.changeModel("parakeetTdt06bV3", { download: { state: "queued" } });
    });

    const small = card("Whisper small");
    expect(within(small).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "30");
    expect(small).toHaveTextContent("30% · 5.2 MB/s");
    expect(card("Parakeet TDT 0.6B v3")).toHaveTextContent("Waiting for the current download");

    // Each ✕ names its Model and what it stops, so a screen reader can tell them apart.
    await userEvent.click(
      within(small).getByRole("button", { name: "Cancel the download of Whisper small" }),
    );
    expect(backend.commandsCalled("cancel_model_download")[0]?.args).toEqual({
      model: "whisperSmall",
    });
    await userEvent.click(
      within(card("Parakeet TDT 0.6B v3")).getByRole("button", {
        name: "Remove Parakeet TDT 0.6B v3 from the queue",
      }),
    );
    expect(backend.commandsCalled("cancel_model_download")[1]?.args).toEqual({
      model: "parakeetTdt06bV3",
    });
  });

  it("shows Verifying, then Paused with Resume and Delete", async () => {
    await openModelsPage();

    act(() => {
      backend.changeModel("whisperSmall", { download: { state: "verifying" } });
    });
    expect(card("Whisper small")).toHaveTextContent("Verifying…");
    expect(within(card("Whisper small")).queryByRole("button")).not.toBeInTheDocument();

    act(() => {
      backend.changeModel("whisperSmall", {
        download: { state: "paused", downloaded: 134_875_568, total: 269_751_136 },
      });
    });
    const small = card("Whisper small");
    expect(small).toHaveTextContent("Paused — 50% downloaded");
    await userEvent.click(within(small).getByRole("button", { name: "Resume" }));
    expect(backend.commandsCalled("download_model")).toHaveLength(1);
    expect(within(small).getByRole("button", { name: "Delete Whisper small" })).toBeVisible();
  });

  it("explains failures in plain language with Retry", async () => {
    await openModelsPage();

    act(() => {
      backend.changeModel("whisperSmall", {
        download: {
          state: "failed",
          failure: { kind: "corrupted", detail: "checksum", neededBytes: null },
          downloaded: 0,
          total: 269_751_136,
        },
      });
      backend.changeModel("parakeetTdt06bV3", {
        download: {
          state: "failed",
          failure: { kind: "diskSpace", detail: "", neededBytes: 844_366_944 },
          downloaded: 0,
          total: 739_508_576,
        },
      });
    });

    expect(within(card("Whisper small")).getByRole("alert")).toHaveTextContent(
      "Download was corrupted — please try again",
    );
    expect(within(card("Parakeet TDT 0.6B v3")).getByRole("alert")).toHaveTextContent(
      "Not enough disk space: 805 MB of free space is needed.",
    );
    await userEvent.click(within(card("Whisper small")).getByRole("button", { name: "Retry" }));
    expect(backend.commandsCalled("download_model")[0]?.args).toEqual({ model: "whisperSmall" });
  });

  it("activates a downloaded Model and marks the active one", async () => {
    downloaded("whisperSmall", "whisperLargeV3Turbo");
    backend.models.active = "whisperLargeV3Turbo";
    backend.models.activeState = "ready";
    await openModelsPage();

    expect(card("Whisper large-v3-turbo")).toHaveTextContent("Active");
    expect(
      within(card("Whisper large-v3-turbo")).queryByRole("button", { name: "Use this Model" }),
    ).not.toBeInTheDocument();

    await userEvent.click(
      within(card("Whisper small")).getByRole("button", { name: "Use this Model" }),
    );
    expect(backend.commandsCalled("activate_model")[0]?.args).toEqual({ model: "whisperSmall" });

    act(() => {
      backend.changeModels({ activating: "whisperSmall" });
    });
    expect(card("Whisper small")).toHaveTextContent("Loading…");
  });

  it("says when a Model could not be loaded", async () => {
    downloaded("whisperSmall");
    backend.models.loadFailure = { model: "whisperSmall", reason: "out of memory" };
    await openModelsPage();

    const alert = within(card("Whisper small")).getByRole("alert");
    expect(alert).toHaveTextContent("Couldn't load Whisper small.");
    expect(alert).not.toHaveTextContent("out of memory");
    expect(within(card("Whisper small")).getByText("Details: out of memory")).toHaveClass(
      "select-text",
    );
  });

  // Rule 20: switching is disabled during Recording and Transcribing.
  it("disables switching and deleting during a Dictation", async () => {
    downloaded("whisperSmall", "whisperLargeV3Turbo");
    backend.models.active = "whisperLargeV3Turbo";
    backend.models.dictationInProgress = true;
    await openModelsPage();

    expect(
      within(card("Whisper small")).getByRole("button", { name: "Use this Model" }),
    ).toBeDisabled();
    expect(
      within(card("Whisper large-v3-turbo")).getByRole("button", {
        name: "Delete Whisper large-v3-turbo",
      }),
    ).toBeDisabled();
    expect(within(screen.getByRole("main")).getByRole("status")).toHaveTextContent(
      "once the Dictation has ended",
    );
  });

  // Rule 25.
  it("deletes only after a confirmation naming the Model and the space freed", async () => {
    downloaded("whisperSmall");
    await openModelsPage();

    await userEvent.click(
      within(card("Whisper small")).getByRole("button", { name: "Delete Whisper small" }),
    );
    const dialog = screen.getByRole("alertdialog", { name: "Delete Whisper small?" });
    expect(dialog).toHaveTextContent("This frees 257 MB");
    await userEvent.click(within(dialog).getByRole("button", { name: "Keep" }));
    expect(backend.commandsCalled("delete_model")).toHaveLength(0);

    await userEvent.click(
      within(card("Whisper small")).getByRole("button", { name: "Delete Whisper small" }),
    );
    await userEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Delete" }),
    );
    expect(backend.commandsCalled("delete_model")[0]?.args).toEqual({ model: "whisperSmall" });
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("offers the unload-after-inactivity choices, Never by default, and saves a change", async () => {
    await openModelsPage();

    const select = screen.getByRole("combobox", { name: /Free the Model's memory when idle/ });
    expect(select).toHaveValue("never");
    expect(
      within(select)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([
      "Never",
      "After 2 minutes",
      "After 5 minutes",
      "After 10 minutes",
      "After 15 minutes",
      "After 60 minutes",
    ]);
    await userEvent.selectOptions(select, "minutes5");
    expect(backend.settings.unloadModelAfter).toBe("minutes5");
  });

  it("is translated into Polish", async () => {
    backend.settings.uiLanguage = "pl";
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Model i język" }));

    expect(await screen.findByRole("region", { name: "Modele" })).toBeVisible();
    expect(card("Whisper large-v3-turbo")).toHaveTextContent("Polecany");
    expect(
      within(card("Whisper small")).getByRole("button", { name: "Pobierz (257 MB)" }),
    ).toBeVisible();
  });
});

describe("Model indicator", () => {
  function indicator() {
    // The status area also holds the updates line, so pick the Model's button by its name:
    // "Open Model settings" without a Model on this computer, the "Model" drop-down with one.
    return within(screen.getByRole("region", { name: "Status" })).getByRole("button", {
      name: /^(Open Model settings|Model):/,
    });
  }

  it("asks for a Model when none is active and opens the Models page", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    expect(indicator()).toHaveTextContent("Download a Model to start");
    expect(screen.getByRole("heading", { name: "Download a Model to start" })).toBeVisible();

    await userEvent.click(indicator());
    expect(
      await screen.findByRole("heading", { level: 1, name: "Model & language" }),
    ).toBeVisible();
  });

  it("follows the active Model's state", async () => {
    downloaded("whisperSmall");
    backend.models.active = "whisperSmall";
    backend.models.activeState = "ready";
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    expect(indicator()).toHaveTextContent("Whisper smallReady");
    expect(screen.queryByRole("heading", { name: "Download a Model to start" })).toBeNull();

    act(() => {
      backend.changeModels({ activeState: "unloaded" });
    });
    expect(indicator()).toHaveTextContent("Freed from memory (loads on next Dictation)");
    act(() => {
      backend.changeModels({ activeState: "error" });
    });
    expect(indicator()).toHaveTextContent("Couldn't load");
    act(() => {
      backend.changeModels({ activating: "whisperLargeV3Turbo" });
    });
    expect(indicator()).toHaveTextContent("Whisper large-v3-turboLoading…");
  });

  // models.md "UI": the sidebar switches between the Models on this computer.
  it("is a drop-down of the downloaded Models, with a way to the Model page", async () => {
    downloaded("whisperSmall", "whisperLargeV3Turbo");
    backend.models.active = "whisperSmall";
    backend.models.activeState = "ready";
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    expect(indicator()).toHaveAttribute("aria-haspopup", "listbox");
    await userEvent.click(indicator());
    const list = screen.getByRole("listbox", { name: "Model" });
    const options = within(list).getAllByRole("option");
    expect(options.map((o) => o.textContent)).toEqual([
      "Whisper large-v3-turbo",
      "Whisper small",
      "Download another Model…",
    ]);
    expect(options[1]).toHaveAttribute("aria-selected", "true");

    await userEvent.click(options[0] as HTMLElement);
    expect(backend.commandsCalled("activate_model").map((c) => c.args)).toEqual([
      { model: "whisperLargeV3Turbo" },
    ]);
    expect(screen.queryByRole("listbox")).toBeNull();

    await userEvent.click(indicator());
    await userEvent.click(screen.getByRole("option", { name: "Download another Model…" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Model & language" }),
    ).toBeVisible();
  });

  it("is chosen from the keyboard too", async () => {
    downloaded("whisperSmall", "whisperLargeV3Turbo");
    backend.models.active = "whisperSmall";
    backend.models.activeState = "ready";
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    indicator().focus();
    await userEvent.keyboard("{ArrowUp}");
    const list = screen.getByRole("listbox", { name: "Model" });
    expect(list).toHaveFocus();
    // Opens on the active Model; Up moves to the other one.
    await userEvent.keyboard("{ArrowUp}{Enter}");
    expect(backend.commandsCalled("activate_model").map((c) => c.args)).toEqual([
      { model: "whisperLargeV3Turbo" },
    ]);
    expect(indicator()).toHaveFocus();

    await userEvent.keyboard("{ArrowDown}");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(backend.commandsCalled("activate_model")).toHaveLength(1);
  });

  it("shows the download progress while no Model is active yet", async () => {
    backend.changeModel("whisperLargeV3Turbo", {
      download: {
        state: "downloading",
        downloaded: 88_638_176,
        total: 886_381_760,
        bytesPerSecond: 0,
      },
    });
    render(<App />);
    await screen.findByRole("navigation", { name: "Sections" });

    expect(indicator()).toHaveTextContent("Downloading 10%");
  });
});
