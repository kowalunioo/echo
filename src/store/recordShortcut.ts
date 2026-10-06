import { create } from "zustand";

import {
  type RecordIntent,
  type RecordShortcutState,
  type ShortcutChangeError,
  type ShortcutMode,
  commands,
} from "../bindings";
import { type CaptureState, captureKey, initialCapture } from "../shortcut/capture";

/** Why the last change of the Record Shortcut was refused; shown next to the field. */
export type ShortcutFeedback =
  | { kind: "rejected"; error: ShortcutChangeError; proposal: string }
  | { kind: "captureUnavailable" };

interface RecordShortcutStore {
  shortcut: RecordShortcutState | null;
  /** Non-null while the shortcut-capture UI is active. */
  capture: CaptureState | null;
  feedback: ShortcutFeedback | null;
  /** TEMPORARY (remove with #13): the last intent, for checking the hook by hand. */
  lastIntent: RecordIntent | null;

  load: () => Promise<void>;
  setMode: (mode: ShortcutMode) => Promise<void>;
  reset: () => Promise<void>;
  beginCapture: () => Promise<void>;
  /** Ends capture without changes (click outside, window lost focus, Escape). */
  endCapture: () => Promise<void>;
  captureKey: (key: string, pressed: boolean) => Promise<void>;
  showIntent: (intent: RecordIntent) => void;
}

export const useRecordShortcut = create<RecordShortcutStore>()((set, get) => {
  async function apply(
    proposal: string,
    change: () => ReturnType<typeof commands.setRecordShortcut>,
  ) {
    const result = await change();
    if (result.status === "ok") {
      set({ shortcut: result.data, feedback: null });
    } else {
      set({ feedback: { kind: "rejected", error: result.error, proposal } });
    }
  }

  return {
    shortcut: null,
    capture: null,
    feedback: null,
    lastIntent: null,

    load: async () => {
      try {
        set({ shortcut: await commands.recordShortcut() });
      } catch (error) {
        console.error("record_shortcut failed", error);
      }
    },

    setMode: async (mode) => {
      set({ shortcut: await commands.setShortcutMode(mode) });
    },

    reset: async () => {
      if (get().capture) await get().endCapture();
      const proposal = get().shortcut?.defaultCombination ?? "";
      await apply(proposal, () => commands.resetRecordShortcut());
    },

    beginCapture: async () => {
      if (get().capture) return;
      set({ capture: initialCapture, feedback: null });
      const result = await commands.beginShortcutCapture();
      if (result.status === "error") {
        console.error("begin_shortcut_capture failed", result.error);
        set({ capture: null, feedback: { kind: "captureUnavailable" } });
      }
    },

    endCapture: async () => {
      if (!get().capture) return;
      set({ capture: null });
      await commands.endShortcutCapture();
    },

    captureKey: async (key, pressed) => {
      const capture = get().capture;
      if (!capture) return;
      const outcome = captureKey(capture, key, pressed);
      switch (outcome.kind) {
        case "continue":
          set({ capture: outcome.state });
          return;
        case "cancel":
          await get().endCapture();
          return;
        case "propose":
          // The backend ends the capture itself before validating the proposal.
          set({ capture: null });
          await apply(outcome.combination, () => commands.setRecordShortcut(outcome.combination));
          return;
      }
    },

    showIntent: (intent) => {
      set({ lastIntent: intent });
    },
  };
});
