import { create } from "zustand";

import { type AppInfo, type UiLanguage, commands } from "../bindings";
import { changeUiLanguage } from "../i18n";

/** The main-window sections from settings-and-first-run.md ("UI"). */
export const PAGES = ["dictation", "model", "vocabulary", "history", "app"] as const;
export type Page = (typeof PAGES)[number];

export type AppInfoState =
  { status: "loading" } | { status: "ready"; info: AppInfo } | { status: "error" };

interface ShellState {
  page: Page;
  uiLanguage: UiLanguage;
  /** True once the user picked a UI Language, so the Windows default no longer applies. */
  uiLanguageChosen: boolean;
  appInfo: AppInfoState;
  setPage: (page: Page) => void;
  chooseUiLanguage: (language: UiLanguage) => Promise<void>;
  loadAppInfo: () => Promise<void>;
}

export const useShell = create<ShellState>()((set, get) => ({
  page: "dictation",
  uiLanguage: "en",
  uiLanguageChosen: false,
  appInfo: { status: "loading" },

  setPage: (page) => {
    set({ page });
  },

  // Not persisted yet: settings persistence arrives with its own slice.
  chooseUiLanguage: async (language) => {
    set({ uiLanguage: language, uiLanguageChosen: true });
    await changeUiLanguage(language);
  },

  loadAppInfo: async () => {
    try {
      const info = await commands.appInfo();
      set({ appInfo: { status: "ready", info } });
      if (!get().uiLanguageChosen) {
        set({ uiLanguage: info.defaultUiLanguage });
        await changeUiLanguage(info.defaultUiLanguage);
      }
    } catch (error) {
      console.error("app_info failed", error);
      set({ appInfo: { status: "error" } });
    }
  },
}));
