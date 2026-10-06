import { create } from "zustand";

import { type AppInfo, commands } from "../bindings";

/** The main-window sections from settings-and-first-run.md ("UI"). */
export const PAGES = ["dictation", "model", "vocabulary", "history", "app"] as const;
export type Page = (typeof PAGES)[number];

export type AppInfoState =
  { status: "loading" } | { status: "ready"; info: AppInfo } | { status: "error" };

interface ShellState {
  page: Page;
  appInfo: AppInfoState;
  setPage: (page: Page) => void;
  loadAppInfo: () => Promise<void>;
}

export const useShell = create<ShellState>()((set) => ({
  page: "dictation",
  appInfo: { status: "loading" },

  setPage: (page) => {
    set({ page });
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
