import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { App } from "../App";
import type { ModelId, ModelLanguages } from "../bindings";
import { changeUiLanguage } from "../i18n";
import { useShell } from "../store/shell";
import { backend, fakeModelLanguages } from "../test/backend";
import { languageOptions, resolveLanguage } from "./dictationLanguage";

const initialShell = useShell.getState();

beforeEach(async () => {
  useShell.setState(initialShell, true);
  await changeUiLanguage("en");
});

function activate(id: ModelId) {
  const entry = backend.models.models.find((m) => m.id === id);
  if (entry) entry.downloaded = true;
  backend.models.active = id;
  backend.models.activeState = "ready";
}

async function openPicker(model: string) {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Model & language" }));
  return screen.findByRole("region", { name: `Language for ${model}` });
}

describe("Dictation Language picker", () => {
  // Acceptance test 7.
  it("lists Automatic, Polish and English first for Whisper", async () => {
    activate("whisperLargeV3Turbo");
    const region = await openPicker("Whisper large-v3-turbo");

    const button = within(region).getByRole("button", { name: /Automatic/ });
    await userEvent.click(button);
    const options = within(region).getAllByRole("option");
    expect(options.slice(0, 3).map((o) => o.textContent)).toEqual([
      "Automatic",
      "Polish",
      "English",
    ]);
    // The rest by name: Chinese, German, Japanese, Spanish, Ukrainian.
    expect(options.slice(3).map((o) => o.textContent)).toEqual([
      "Chinese",
      "German",
      "Japanese",
      "Spanish",
      "Ukrainian",
    ]);
    expect(options[0]).toHaveAttribute("aria-selected", "true");
  });

  it("replaces the picker with a note for Parakeet", async () => {
    activate("parakeetTdt06bV3");
    const region = await openPicker("Parakeet TDT 0.6B v3");
    expect(region).toHaveTextContent("This Model detects the language automatically.");
    expect(within(region).queryByRole("button")).toBeNull();
  });

  it("filters as the user types and Enter picks the first match", async () => {
    activate("whisperLargeV3Turbo");
    const region = await openPicker("Whisper large-v3-turbo");
    await userEvent.click(within(region).getByRole("button", { name: /Automatic/ }));

    await userEvent.type(within(region).getByRole("searchbox"), "germ");
    expect(
      within(region)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["German"]);
    await userEvent.keyboard("{Enter}");

    expect(backend.settings.dictationLanguage).toBe("de");
    expect(within(region).queryByRole("listbox")).toBeNull();
    expect(within(region).getByRole("button", { name: /German/ })).toBeVisible();
  });

  it("moves through the list with the arrow keys, Home and End, and Enter picks", async () => {
    activate("whisperLargeV3Turbo");
    const region = await openPicker("Whisper large-v3-turbo");
    await userEvent.click(within(region).getByRole("button", { name: /Automatic/ }));
    const search = within(region).getByRole("searchbox");
    const activeOption = () => {
      const id = search.getAttribute("aria-activedescendant");
      return id ? document.getElementById(id)?.textContent : null;
    };

    // Opens on the current language.
    expect(activeOption()).toBe("Automatic");
    await userEvent.keyboard("{ArrowUp}");
    expect(activeOption()).toBe("Automatic");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    expect(activeOption()).toBe("English");
    await userEvent.keyboard("{End}");
    expect(activeOption()).toBe("Ukrainian");
    await userEvent.keyboard("{ArrowDown}");
    expect(activeOption()).toBe("Ukrainian");
    await userEvent.keyboard("{Home}{ArrowDown}");
    expect(activeOption()).toBe("Polish");

    await userEvent.keyboard("{Enter}");
    expect(backend.settings.dictationLanguage).toBe("pl");
    expect(within(region).queryByRole("listbox")).toBeNull();
    expect(within(region).getByRole("button", { name: /Polish/ })).toHaveFocus();
  });

  it("opens on the stored language and ArrowDown on the button opens the list", async () => {
    backend.settings.dictationLanguage = "ja";
    activate("whisperLargeV3Turbo");
    const region = await openPicker("Whisper large-v3-turbo");
    within(region)
      .getByRole("button", { name: /Japanese/ })
      .focus();
    await userEvent.keyboard("{ArrowDown}");

    const search = within(region).getByRole("searchbox");
    const id = search.getAttribute("aria-activedescendant");
    expect(id && document.getElementById(id)).toHaveTextContent("Japanese");
  });

  it("Escape closes the list without changing the language", async () => {
    activate("whisperLargeV3Turbo");
    const region = await openPicker("Whisper large-v3-turbo");
    await userEvent.click(within(region).getByRole("button", { name: /Automatic/ }));
    await userEvent.keyboard("{Escape}");
    expect(within(region).queryByRole("listbox")).toBeNull();
    expect(backend.settings.dictationLanguage).toBe("automatic");
  });

  it("explains the fallback for a language the Model lacks and resets to Automatic", async () => {
    backend.settings.dictationLanguage = "ja";
    activate("whisperSmall");
    const region = await openPicker("Whisper small");

    expect(within(region).getByRole("button", { name: /: Automatic$/ })).toBeVisible();
    expect(region).toHaveTextContent(
      "Japanese is not available for this Model — Automatic will be used.",
    );
    await userEvent.click(within(region).getByRole("button", { name: "Reset to Automatic" }));
    expect(backend.settings.dictationLanguage).toBe("automatic");
    expect(region).not.toHaveTextContent("not available");
  });

  it("names the languages in the UI Language", async () => {
    backend.settings.uiLanguage = "pl";
    backend.settings.dictationLanguage = "pl";
    activate("whisperLargeV3Turbo");
    await changeUiLanguage("pl");
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Model i język" }));
    const region = await screen.findByRole("region", {
      name: "Język dla modelu Whisper large-v3-turbo",
    });
    await userEvent.click(within(region).getByRole("button", { name: /Polski/ }));
    expect(
      within(region)
        .getAllByRole("option")
        .slice(0, 4)
        .map((o) => o.textContent),
    ).toEqual(["Automatycznie", "Polski", "Angielski", "Chiński"]);
  });
});

describe("Dictation Language rules", () => {
  const [whisper, parakeet] = fakeModelLanguages() as [ModelLanguages, ModelLanguages];
  const englishOnly = {
    model: whisper.model,
    languages: ["de", "en"],
    automatic: false,
    honoursLanguage: true,
  };

  it("resolves the intent against the Model like the backend (rule 4)", () => {
    expect(resolveLanguage("pl", whisper)).toBe("pl");
    expect(resolveLanguage("ja", parakeet)).toBe("automatic");
    expect(resolveLanguage("pl", englishOnly)).toBe("en");
    expect(resolveLanguage("automatic", { ...englishOnly, languages: ["fr", "de"] })).toBe("fr");
    expect(resolveLanguage("en", { ...englishOnly, languages: ["en-US"] })).toBe("en-US");
    expect(resolveLanguage("no", { ...englishOnly, languages: ["nb-NO"] })).toBe("nb-NO");
  });

  it("omits Automatic for a Model that cannot detect", () => {
    expect(languageOptions(englishOnly, "en")).toEqual(["en", "de"]);
  });
});
