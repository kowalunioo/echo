import { create } from "zustand";

import { type HistoryEntry, commands, events } from "../bindings";

/**
 * The frontend's copy of History (history.md). The backend owns it: every change, wherever it
 * comes from (a Dictation, the limit, another window), arrives as a `historyChanged` event with
 * the complete list, newest first (rule 16).
 */
export type HistoryState =
  | { status: "loading"; entries: [] }
  | { status: "ready"; entries: HistoryEntry[] }
  | { status: "error"; entries: [] };

interface HistoryActions {
  /** Fetches the entries and starts following changes. */
  load: () => Promise<void>;
  /** Deletes one entry at once and returns it for Undo, or null if it could not be deleted. */
  remove: (id: number) => Promise<HistoryEntry | null>;
  /** Puts a deleted entry back with its original time (Undo). */
  restore: (entry: HistoryEntry) => Promise<void>;
  /** Deletes every entry. */
  clear: () => Promise<void>;
  /** Hides the window and inserts the entry's text where the user was typing; false on failure. */
  reinsert: (id: number) => Promise<boolean>;
}

let stopListening: (() => void) | undefined;

export const useHistory = create<HistoryState & HistoryActions>()((set) => ({
  status: "loading",
  entries: [],

  load: async () => {
    if (!stopListening) {
      const unlisten = events.historyChanged.listen((event) => {
        set({ status: "ready", entries: event.payload });
      });
      stopListening = () => {
        void unlisten.then((stop) => {
          stop();
        });
      };
    }
    const result = await commands.listHistory();
    if (result.status === "ok") {
      set({ status: "ready", entries: result.data });
    } else {
      console.error("list_history failed", result.error);
      set({ status: "error", entries: [] });
    }
  },

  remove: async (id) => {
    const result = await commands.deleteHistoryEntry(id);
    if (result.status === "ok") return result.data;
    console.error("delete_history_entry failed", result.error);
    return null;
  },

  restore: async (entry) => {
    const result = await commands.restoreHistoryEntry(entry);
    if (result.status === "error") console.error("restore_history_entry failed", result.error);
  },

  clear: async () => {
    const result = await commands.clearHistory();
    if (result.status === "error") console.error("clear_history failed", result.error);
  },

  reinsert: async (id) => {
    const result = await commands.reinsertHistoryEntry(id);
    if (result.status === "error") console.error("reinsert_history_entry failed", result.error);
    return result.status === "ok";
  },
}));

/** Stops following History changes; for tests that start each case from scratch. */
export function resetHistoryStore() {
  stopListening?.();
  stopListening = undefined;
  useHistory.setState({ status: "loading", entries: [] });
}
