import { describe, expect, it } from "vitest";

import {
  type CaptureOutcome,
  type CaptureState,
  captureKey,
  heldCombination,
  initialCapture,
  keysOf,
} from "./capture";

type Step = [key: string, pressed: boolean];

/** Runs key steps through the capture; returns the final outcome and the last state. */
function run(steps: Step[]): { outcome: CaptureOutcome | null; state: CaptureState } {
  let state = initialCapture;
  let outcome: CaptureOutcome | null = null;
  for (const [key, pressed] of steps) {
    outcome = captureKey(state, key, pressed);
    if (outcome.kind !== "continue") return { outcome, state };
    state = outcome.state;
  }
  return { outcome, state };
}

const down = (key: string): Step => [key, true];
const up = (key: string): Step => [key, false];

describe("shortcut capture", () => {
  // record-shortcut.md acceptance test 14 (key part).
  it("proposes Ctrl+Space when the main key is released", () => {
    expect(run([down("LeftCtrl"), down("Space"), up("Space")]).outcome).toEqual({
      kind: "propose",
      combination: "Ctrl+Space",
    });
  });

  it("proposes Ctrl+Win when all modifiers of a modifier-only combination are released", () => {
    const steps = [down("LeftCtrl"), down("LeftWin"), up("LeftWin")];
    expect(run(steps).outcome?.kind).toBe("continue");
    expect(run([...steps, up("LeftCtrl")]).outcome).toEqual({
      kind: "propose",
      combination: "Ctrl+Win",
    });
  });

  it("writes combinations in canonical order whatever the press order", () => {
    expect(
      run([down("RightShift"), down("LeftAlt"), down("RightCtrl"), down("D"), up("D")]).outcome,
    ).toEqual({ kind: "propose", combination: "Ctrl+Alt+Shift+D" });
  });

  it("treats left and right modifiers alike in combinations", () => {
    expect(
      run([down("RightCtrl"), down("LeftShift"), up("LeftShift"), up("RightCtrl")]).outcome,
    ).toEqual({ kind: "propose", combination: "Ctrl+Shift" });
  });

  it("keeps right Alt and right Ctrl alone side-specific", () => {
    expect(run([down("RightAlt"), up("RightAlt")]).outcome).toEqual({
      kind: "propose",
      combination: "RightAlt",
    });
    expect(run([down("RightCtrl"), up("RightCtrl")]).outcome).toEqual({
      kind: "propose",
      combination: "RightCtrl",
    });
    // Left Ctrl alone is proposed as plain Ctrl, which validation then rejects.
    expect(run([down("LeftCtrl"), up("LeftCtrl")]).outcome).toEqual({
      kind: "propose",
      combination: "Ctrl",
    });
  });

  it("proposes a single key alone, leaving validation to the backend", () => {
    expect(run([down("F9"), up("F9")]).outcome).toEqual({ kind: "propose", combination: "F9" });
    expect(run([down("Space"), up("Space")]).outcome).toEqual({
      kind: "propose",
      combination: "Space",
    });
  });

  it("keeps modifiers released before the main key in the combination", () => {
    expect(run([down("LeftCtrl"), down("Space"), up("LeftCtrl"), up("Space")]).outcome).toEqual({
      kind: "propose",
      combination: "Ctrl+Space",
    });
  });

  it("ends without changes when Escape is pressed alone", () => {
    expect(run([down("Escape")]).outcome).toEqual({ kind: "cancel" });
  });

  it("uses Escape like any key when modifiers are held", () => {
    expect(run([down("LeftCtrl"), down("Escape"), up("Escape")]).outcome).toEqual({
      kind: "propose",
      combination: "Ctrl+Escape",
    });
  });

  it("ignores auto-repeat and releases of keys held before capture began", () => {
    expect(
      run([up("LeftShift"), down("LeftCtrl"), down("LeftCtrl"), down("Space"), up("Space")])
        .outcome,
    ).toEqual({ kind: "propose", combination: "Ctrl+Space" });
  });

  it("shows the keys as they are held", () => {
    expect(heldCombination(run([down("LeftWin"), down("RightCtrl")]).state)).toBe("Ctrl+Win");
    expect(heldCombination(run([down("RightAlt")]).state)).toBe("RightAlt");
    expect(heldCombination(run([down("LeftCtrl"), down("Space")]).state)).toBe("Ctrl+Space");
    expect(heldCombination(initialCapture)).toBe("");
  });

  it("splits a combination into key caps", () => {
    expect(keysOf("Ctrl+Alt+D")).toEqual(["Ctrl", "Alt", "D"]);
    expect(keysOf("")).toEqual([]);
  });
});
