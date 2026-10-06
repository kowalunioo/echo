import { create } from "zustand";

import { type ShortcutChangeError, commands } from "../bindings";
import { type CaptureState, captureKey, initialCapture } from "../shortcut/capture";
import { useSettings } from "./settings";

/**
 * The capture state and feedback of the Record Shortcut and the Cancel Shortcut. The
 * combinations and the mode themselves are settings (`recordShortcut`, `cancelShortcut`,
 * `shortcutMode`; read them with `useSetting`). A captured combination goes through
 * `set_record_shortcut` or `set_cancel_shortcut`, which check it before saving it and say why
 * when they cannot take it (record-shortcut.md rules 20–23, cancel-shortcut.md rule 11).
 */

/** The default Record Shortcut, restored by "Reset to default" (record-shortcut.md rule 18). */
export const DEFAULT_RECORD_SHORTCUT = "Ctrl+Space";

/** The default Cancel Shortcut, restored by "Reset to default" (cancel-shortcut.md rule 10). */
export const DEFAULT_CANCEL_SHORTCUT = "Escape";

/** Which shortcut a capture or a message is about. */
export type ShortcutTarget = "record" | "cancel";

/** The setting that holds each shortcut. */
export const SETTING_OF = {
  record: "recordShortcut",
  cancel: "cancelShortcut",
} as const satisfies Record<ShortcutTarget, string>;

/** Why the last change of a shortcut was refused; shown next to its field. */
export type ShortcutFeedback = { target: ShortcutTarget } & (
  | { kind: "rejected"; error: ShortcutChangeError; proposal: string }
  | { kind: "captureUnavailable" }
);

interface RecordShortcutStore {
  /** Non-null while the shortcut-capture UI is active. */
  capture: CaptureState | null;
  /** The shortcut being captured. */
  target: ShortcutTarget;
  feedback: ShortcutFeedback | null;

  reset: (target?: ShortcutTarget) => Promise<void>;
  beginCapture: (target?: ShortcutTarget) => Promise<void>;
  /** Ends capture without changes (click outside, window lost focus, Escape). */
  endCapture: () => Promise<void>;
  captureKey: (key: string, pressed: boolean) => Promise<void>;
}

export const useRecordShortcut = create<RecordShortcutStore>()((set, get) => ({
  capture: null,
  target: "record",
  feedback: null,

  reset: async (target = "record") => {
    await get().endCapture();
    set({ feedback: null });
    await useSettings.getState().reset(SETTING_OF[target]);
  },

  beginCapture: async (target = "record") => {
    if (get().capture) return;
    set({ capture: initialCapture, target, feedback: null });
    const result = await commands.beginShortcutCapture();
    if (result.status === "error") {
      console.error("begin_shortcut_capture failed", result.error);
      set({ capture: null, feedback: { target, kind: "captureUnavailable" } });
    }
  },

  endCapture: async () => {
    if (!get().capture) return;
    set({ capture: null });
    await commands.endShortcutCapture();
  },

  captureKey: async (key, pressed) => {
    const { capture, target } = get();
    if (!capture) return;
    // Escape alone is a valid Cancel Shortcut, so it does not end its capture.
    const outcome = captureKey(capture, key, pressed, { escapeIsKey: target === "cancel" });
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
        const result =
          target === "cancel"
            ? await commands.setCancelShortcut(proposal)
            : await commands.setRecordShortcut(proposal);
        if (result.status === "ok") {
          useSettings.setState({ status: "ready", settings: result.data });
          set({ feedback: null });
        } else {
          set({ feedback: { target, kind: "rejected", error: result.error, proposal } });
        }
        return;
      }
    }
  },
}));

/** Forgets capture and feedback; for tests that start each case from scratch. */
export function resetRecordShortcutStore() {
  useRecordShortcut.setState({ capture: null, target: "record", feedback: null });
}
