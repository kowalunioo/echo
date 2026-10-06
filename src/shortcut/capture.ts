/**
 * Shortcut capture (record-shortcut.md, "UI"): turns the keys the user presses and releases into
 * a proposed combination in canonical text form ("Ctrl+Space", "Ctrl+Win", "F9").
 *
 * Keys arrive by their capture names from the keyboard hook: side-specific for modifiers
 * ("LeftCtrl", "RightAlt", …) and canonical names for other keys ("Space", "F9", "Escape").
 * Validation happens in the backend; this only decides what is proposed and when.
 */

export type Modifier = "Ctrl" | "Alt" | "Shift" | "Win";

const MODIFIER_ORDER: readonly Modifier[] = ["Ctrl", "Alt", "Shift", "Win"];

const MODIFIER_OF: Readonly<Record<string, Modifier | undefined>> = {
  LeftCtrl: "Ctrl",
  RightCtrl: "Ctrl",
  LeftAlt: "Alt",
  RightAlt: "Alt",
  LeftShift: "Shift",
  RightShift: "Shift",
  LeftWin: "Win",
  RightWin: "Win",
};

export interface CaptureState {
  /** Keys currently held, by capture name, in the order they were pressed. */
  held: readonly string[];
  /** The first non-modifier key pressed, if any. */
  main: string | null;
  /** Modifiers that belong to the main key's combination. */
  mainModifiers: readonly Modifier[];
  /** For a modifier-only combination: the modifier keys held when the first one was released. */
  peak: readonly string[] | null;
}

export type CaptureOutcome =
  | { kind: "continue"; state: CaptureState }
  | { kind: "propose"; combination: string }
  | { kind: "cancel" };

export const initialCapture: CaptureState = { held: [], main: null, mainModifiers: [], peak: null };

function modifiersOf(keys: readonly string[]): Modifier[] {
  const kinds = new Set(keys.map((k) => MODIFIER_OF[k]));
  return MODIFIER_ORDER.filter((m) => kinds.has(m));
}

function isModifier(key: string): boolean {
  return MODIFIER_OF[key] !== undefined;
}

/** Canonical text form: Ctrl, Alt, Shift, Win, then the main key (rule 21). */
export function canonical(modifiers: readonly Modifier[], main: string | null): string {
  return [...MODIFIER_ORDER.filter((m) => modifiers.includes(m)), ...(main ? [main] : [])].join(
    "+",
  );
}

/** What the held keys look like right now, in canonical form, for showing during capture. */
export function heldCombination(state: CaptureState): string {
  return canonical(modifiersOf(state.held.filter(isModifier)), state.main);
}

export interface CaptureOptions {
  /**
   * Escape alone is a value to capture (the Cancel Shortcut, cancel-shortcut.md "UI") rather
   * than the way to end capture without changes (the Record Shortcut).
   */
  escapeIsKey?: boolean;
}

/** Feeds one key press or release into the capture. */
export function captureKey(
  state: CaptureState,
  key: string,
  pressed: boolean,
  options: CaptureOptions = {},
): CaptureOutcome {
  if (pressed) {
    // Escape alone ends capture without changes, unless it is a value to capture.
    if (key === "Escape" && state.held.length === 0 && !options.escapeIsKey) {
      return { kind: "cancel" };
    }
    if (state.held.includes(key)) return { kind: "continue", state };
    const held = [...state.held, key];
    if (isModifier(key)) {
      const mainModifiers = state.main ? modifiersOf([...state.mainModifiers, key]) : [];
      return { kind: "continue", state: { ...state, held, mainModifiers } };
    }
    if (state.main) return { kind: "continue", state: { ...state, held } };
    return {
      kind: "continue",
      state: { ...state, held, main: key, mainModifiers: modifiersOf(state.held) },
    };
  }

  if (!state.held.includes(key)) return { kind: "continue", state };
  const held = state.held.filter((k) => k !== key);
  if (key === state.main) {
    return { kind: "propose", combination: canonical(state.mainModifiers, state.main) };
  }
  if (state.main || !isModifier(key)) return { kind: "continue", state: { ...state, held } };

  const peak = state.peak ?? state.held.filter(isModifier);
  if (held.some(isModifier)) return { kind: "continue", state: { ...state, held, peak } };
  // A single modifier, right Alt and right Ctrl included, is proposed as the plain modifier,
  // which validation rejects (issue #33).
  return { kind: "propose", combination: canonical(modifiersOf(peak), null) };
}

/** Splits a canonical combination into its keys, for showing as key caps. */
export function keysOf(combination: string): string[] {
  return combination === "" ? [] : combination.split("+");
}
