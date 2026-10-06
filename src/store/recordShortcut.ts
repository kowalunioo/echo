import { create } from "zustand";

import { type ShortcutChangeError, commands } from "../bindings";
import { type CaptureState, captureKey, initialCapture } from "../shortcut/capture";
import { useSettings } from "./settings";

/**
 * The Record Shortcut's capture state and feedback. The combination and mode themselves are
 * settings (`recordShortcut`, `shortcutMode`; read them with `useSetting`). A captured
 * combination goes through `set_record_shortcut`, which activates it before saving it and says
 * why when it cannot (record-shortcut.md rules 20–23).
 */

/** The default Record Shortcut, restored by "Reset to default" (record-shortcut.md rule 18). */
export const DEFAULT_RECORD_SHORTCUT = "Ctrl+Space";

/** Why the last change of the Record Shortcut was refused; shown next to the field. */
export type ShortcutFeedback =
  | { kind: "rejected"; error: ShortcutChangeError; proposal: string }
  | { kind: "captureUnavailable" };

interface RecordShortcutStore {
  /** Non-null while the shortcut-capture UI is active. */
  capture: CaptureState | null;
  feedback: ShortcutFeedback | null;

  reset: () => Promise<void>;
  beginCapture: () => Promise<void>;
  /** Ends capture without changes (click outside, window lost focus, Escape). */
  endCapture: () => Promise<void>;
  captureKey: (key: string, pressed: boolean) => Promise<void>;
}

export const useRecordShortcut = create<RecordShortcutStore>()((set, get) => ({
  capture: null,
  feedback: null,

  reset: async () => {
    await get().endCapture();
    set({ feedback: null });
    await useSettings.getState().reset("recordShortcut");
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
      case "propose": {
        // The backend ends the capture itself before validating the proposal.
        set({ capture: null });
        const proposal = outcome.combination;
        const result = await commands.setRecordShortcut(proposal);
        if (result.status === "ok") {
          useSettings.setState({ status: "ready", settings: result.data });
          set({ feedback: null });
        } else {
          set({ feedback: { kind: "rejected", error: result.error, proposal } });
        }
        return;
      }
    }
  },
}));

/** Forgets capture and feedback; for tests that start each case from scratch. */
export function resetRecordShortcutStore() {
  useRecordShortcut.setState({ capture: null, feedback: null });
}
