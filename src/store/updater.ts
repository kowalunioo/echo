import { create } from "zustand";

import { type UpdaterView, commands, events } from "../bindings";

/**
 * The frontend's copy of the updater's view (updater.md "UI"): version, whether updates are
 * managed by the system, the status line and the "updated to" notice. The backend owns it and
 * sends every change as an `updaterChanged` event.
 */
interface UpdaterStore {
  /** `null` until the first load. */
  view: UpdaterView | null;
  /** Fetches the view and starts following changes. Safe to call more than once. */
  load: () => Promise<void>;
  check: () => Promise<void>;
  /** "Install and restart". */
  install: () => Promise<void>;
  dismissNotice: () => Promise<void>;
}

let listening = false;

export const useUpdater = create<UpdaterStore>()((set) => ({
  view: null,

  load: async () => {
    if (!listening) {
      listening = true;
      events.updaterChanged
        .listen((event) => {
          set({ view: event.payload });
        })
        .catch((error: unknown) => {
          listening = false;
          console.error("cannot listen for the updater", error);
        });
    }
    try {
      set({ view: await commands.getUpdaterView() });
    } catch (error) {
      console.error("get_updater_view failed", error);
    }
  },

  check: async () => {
    await commands.checkForUpdates();
  },

  install: async () => {
    await commands.installUpdate();
  },

  dismissNotice: async () => {
    await commands.dismissUpdateNotice();
  },
}));

/** Forgets the view; for tests that start each case from scratch. */
export function resetUpdaterStore() {
  listening = false;
  useUpdater.setState({ view: null });
}
