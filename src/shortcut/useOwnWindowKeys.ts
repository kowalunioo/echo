import { useEffect } from "react";

import { commands } from "../bindings";
import { useRecordShortcut } from "../store/recordShortcut";
import { createKeyForwarder } from "./ownWindowKeys";

/** Forwards every key event of Echo's window to the backend (see `ownWindowKeys.ts`). */
export function useOwnWindowKeys() {
  useEffect(() => {
    const forward = createKeyForwarder();
    // Separate invokes may be handled out of order, and order matters (Ctrl before D), so each
    // key is sent only after the previous one was handled.
    let queue = Promise.resolve();
    const onKey = (event: KeyboardEvent) => {
      // During capture the keys belong to the capture, not to the focused button or the page.
      if (useRecordShortcut.getState().capture) event.preventDefault();
      for (const { key, pressed } of forward(event)) {
        queue = queue
          .then(() => commands.ownWindowKey(key, pressed))
          .catch((error: unknown) => {
            console.error("own_window_key failed", error);
          });
      }
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("keyup", onKey, true);
    };
  }, []);
}
