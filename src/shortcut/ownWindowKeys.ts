/**
 * Keys pressed in Echo's own window. Windows does not run Echo's keyboard hook while Echo's
 * window has focus, so the window forwards its key events to the backend, which matches them
 * like hook keystrokes: the Record Shortcut then works with Echo in front, and shortcut capture
 * receives the keys typed into it.
 */

const NAMED: Readonly<Record<string, string | undefined>> = {
  ControlLeft: "LeftCtrl",
  ControlRight: "RightCtrl",
  AltLeft: "LeftAlt",
  AltRight: "RightAlt",
  ShiftLeft: "LeftShift",
  ShiftRight: "RightShift",
  MetaLeft: "LeftWin",
  MetaRight: "RightWin",
  OSLeft: "LeftWin",
  OSRight: "RightWin",
  Space: "Space",
  Enter: "Enter",
  NumpadEnter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Escape: "Escape",
  Insert: "Insert",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowLeft: "Left",
  ArrowUp: "Up",
  ArrowRight: "Right",
  ArrowDown: "Down",
  Pause: "Pause",
  ScrollLock: "ScrollLock",
  CapsLock: "CapsLock",
  NumLock: "NumLock",
  PrintScreen: "PrintScreen",
  ContextMenu: "ContextMenu",
  NumpadMultiply: "NumpadMultiply",
  NumpadAdd: "NumpadAdd",
  NumpadSubtract: "NumpadSubtract",
  NumpadDecimal: "NumpadDecimal",
  NumpadDivide: "NumpadDivide",
  Semicolon: "Semicolon",
  Equal: "Equal",
  Comma: "Comma",
  Minus: "Minus",
  Period: "Period",
  Slash: "Slash",
  Backquote: "Backquote",
  BracketLeft: "BracketLeft",
  Backslash: "Backslash",
  BracketRight: "BracketRight",
  Quote: "Quote",
  IntlBackslash: "IntlBackslash",
};

/** The capture name of a DOM `KeyboardEvent.code` (layout-independent), if Echo knows the key. */
export function captureNameOf(code: string): string | null {
  const named = NAMED[code];
  if (named) return named;
  let match = /^Key([A-Z])$/.exec(code) ?? /^Digit([0-9])$/.exec(code);
  if (match) return match[1] ?? null;
  match = /^Numpad([0-9])$/.exec(code);
  if (match) return `Numpad${match[1] ?? ""}`;
  match = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code);
  if (match) return code;
  return null;
}

const MODIFIER_KEYS: Readonly<Record<string, string | undefined>> = {
  Control: "Ctrl",
  Alt: "Alt",
  AltGraph: "Alt",
  Shift: "Shift",
  Meta: "Win",
  OS: "Win",
};

/**
 * The capture name of a key event. Uses the layout-independent `code`; input without a scan code
 * (some remote-desktop and synthetic input) has an empty `code`, so it falls back to `key` and
 * `location` for modifiers, letters, digits, F-keys and Space.
 */
export function captureNameOfEvent(
  event: Pick<KeyboardEvent, "code" | "key" | "location">,
): string | null {
  if (event.code) return captureNameOf(event.code);
  const modifier = MODIFIER_KEYS[event.key];
  if (modifier) return `${event.location === 2 ? "Right" : "Left"}${modifier}`;
  if (event.key === " ") return "Space";
  if (/^[a-z0-9]$/i.test(event.key)) return event.key.toUpperCase();
  return captureNameOf(event.key);
}

export interface ForwardedKey {
  key: string;
  pressed: boolean;
}

/**
 * Turns the window's key events into forwarded keys. On layouts with AltGr (e.g. Polish),
 * Windows reports right Alt as a synthetic left Ctrl press followed by right Alt; the synthetic
 * Ctrl is withdrawn so right Alt alone stays right Alt.
 */
export function createKeyForwarder() {
  let ctrlFromAltGr = false;
  let lastCtrlDown: number | null = null;

  return (
    event: Pick<KeyboardEvent, "code" | "key" | "location" | "type" | "timeStamp">,
  ): ForwardedKey[] => {
    const key = captureNameOfEvent(event);
    if (!key) return [];
    const pressed = event.type === "keydown";

    if (key === "LeftCtrl") {
      if (pressed) {
        lastCtrlDown = event.timeStamp;
        return [{ key, pressed }];
      }
      if (ctrlFromAltGr) {
        ctrlFromAltGr = false;
        return [];
      }
      lastCtrlDown = null;
      return [{ key, pressed }];
    }

    if (
      key === "RightAlt" &&
      pressed &&
      event.key === "AltGraph" &&
      lastCtrlDown !== null &&
      event.timeStamp - lastCtrlDown < 50
    ) {
      ctrlFromAltGr = true;
      lastCtrlDown = null;
      return [
        { key: "LeftCtrl", pressed: false },
        { key, pressed },
      ];
    }
    return [{ key, pressed }];
  };
}
