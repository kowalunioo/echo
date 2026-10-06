import { create } from "zustand";

import { type DictationStatus, commands, events } from "../bindings";

/**
 * The frontend's copy of the dictation status (dictation-pipeline.md rules 39b–39d): state, the
 * tray error and the error notices for the main window. The backend owns it and sends every
 * change as a `dictationStatusChanged` event.
 */
interface DictationStore {
  status: DictationStatus;
  /** Fetches the status and starts following changes. Safe to call more than once. */
  load: () => Promise<void>;
  /** The main window is in front of the user: the tray error clears. */
  seen: () => Promise<void>;
  dismiss: () => Promise<void>;
}

export const IDLE_STATUS: DictationStatus = {
  state: "idle",
  listening: false,
  error: null,
  notices: [],
};

let listening = false;

export const useDictation = create<DictationStore>()((set) => ({
  status: IDLE_STATUS,

  load: async () => {
    if (!listening) {
      listening = true;
      events.dictationStatusChanged
        .listen((event) => {
          set({ status: event.payload });
        })
        .catch((error: unknown) => {
          listening = false;
          console.error("cannot listen for the dictation status", error);
        });
    }
    try {
      set({ status: await commands.getDictationStatus() });
    } catch (error) {
      console.error("get_dictation_status failed", error);
    }
  },

  seen: async () => {
    await commands.dictationWindowSeen();
  },

  dismiss: async () => {
    await commands.dismissDictationNotices();
  },
}));

/** Forgets the status; for tests that start each case from scratch. */
export function resetDictationStore() {
  listening = false;
  useDictation.setState({ status: IDLE_STATUS });
}
