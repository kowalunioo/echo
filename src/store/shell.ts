import { create } from "zustand";

import { type AppInfo, commands } from "../bindings";

/** The main-window sections from settings-and-first-run.md ("UI"). */
export const PAGES = ["dictation", "model", "vocabulary", "history", "app"] as const;
export type Page = (typeof PAGES)[number];

export type AppInfoState =
  { status: "loading" } | { status: "ready"; info: AppInfo } | { status: "error" };

/** Sections another part of the window can send the user to, e.g. from an error notice. */
export type Section = "microphone";

interface ShellState {
  page: Page;
  /** A section waiting to take focus once its page shows it; cleared when it has. */
  section: Section | null;
  appInfo: AppInfoState;
  setPage: (page: Page) => void;
  /** Shows `page` and moves focus to `section` on it. */
  openSection: (page: Page, section: Section) => void;
  /** The section took focus. */
  sectionShown: () => void;
  loadAppInfo: () => Promise<void>;
}

export const useShell = create<ShellState>()((set) => ({
  page: "dictation",
  section: null,
  appInfo: { status: "loading" },

  setPage: (page) => {
    set({ page, section: null });
  },

  openSection: (page, section) => {
    set({ page, section });
  },

  sectionShown: () => {
    set({ section: null });
  },

  loadAppInfo: async () => {
    try {
      const info = await commands.appInfo();
      set({ appInfo: { status: "ready", info } });
    } catch (error) {
      console.error("app_info failed", error);
      set({ appInfo: { status: "error" } });
    }
  },
}));
