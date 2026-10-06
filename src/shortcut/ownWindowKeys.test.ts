import { describe, expect, it } from "vitest";

import { captureNameOf, captureNameOfEvent, createKeyForwarder } from "./ownWindowKeys";

describe("own window keys", () => {
  it("maps DOM key codes to capture names", () => {
    expect(captureNameOf("ControlLeft")).toBe("LeftCtrl");
    expect(captureNameOf("AltRight")).toBe("RightAlt");
    expect(captureNameOf("MetaLeft")).toBe("LeftWin");
    expect(captureNameOf("KeyD")).toBe("D");
    expect(captureNameOf("Digit7")).toBe("7");
    expect(captureNameOf("Numpad5")).toBe("Numpad5");
    expect(captureNameOf("F9")).toBe("F9");
    expect(captureNameOf("F24")).toBe("F24");
    expect(captureNameOf("Space")).toBe("Space");
    expect(captureNameOf("ArrowUp")).toBe("Up");
    expect(captureNameOf("F25")).toBeNull();
    expect(captureNameOf("MediaPlayPause")).toBeNull();
  });

  it("falls back to key and location when the event has no code", () => {
    expect(captureNameOfEvent({ code: "", key: "Control", location: 1 })).toBe("LeftCtrl");
    expect(captureNameOfEvent({ code: "", key: "Alt", location: 2 })).toBe("RightAlt");
    expect(captureNameOfEvent({ code: "", key: "Meta", location: 1 })).toBe("LeftWin");
    expect(captureNameOfEvent({ code: "", key: " ", location: 0 })).toBe("Space");
    expect(captureNameOfEvent({ code: "", key: "d", location: 0 })).toBe("D");
    expect(captureNameOfEvent({ code: "", key: "F9", location: 0 })).toBe("F9");
    expect(captureNameOfEvent({ code: "KeyQ", key: "a", location: 0 })).toBe("Q");
  });

  it("forwards presses and releases", () => {
    const forward = createKeyForwarder();
    expect(
      forward({ location: 0, code: "ControlLeft", key: "Control", type: "keydown", timeStamp: 0 }),
    ).toEqual([{ key: "LeftCtrl", pressed: true }]);
    expect(
      forward({ location: 0, code: "Space", key: " ", type: "keydown", timeStamp: 100 }),
    ).toEqual([{ key: "Space", pressed: true }]);
    expect(
      forward({ location: 0, code: "ControlLeft", key: "Control", type: "keyup", timeStamp: 200 }),
    ).toEqual([{ key: "LeftCtrl", pressed: false }]);
  });

  it("withdraws the left Ctrl that AltGr synthesises", () => {
    const forward = createKeyForwarder();
    const events = [
      forward({ location: 0, code: "ControlLeft", key: "Control", type: "keydown", timeStamp: 10 }),
      forward({ location: 0, code: "AltRight", key: "AltGraph", type: "keydown", timeStamp: 10 }),
      forward({ location: 0, code: "ControlLeft", key: "Control", type: "keyup", timeStamp: 90 }),
      forward({ location: 0, code: "AltRight", key: "AltGraph", type: "keyup", timeStamp: 90 }),
    ].flat();
    expect(events).toEqual([
      { key: "LeftCtrl", pressed: true },
      { key: "LeftCtrl", pressed: false },
      { key: "RightAlt", pressed: true },
      { key: "RightAlt", pressed: false },
    ]);
  });

  it("keeps a real left Ctrl held well before right Alt", () => {
    const forward = createKeyForwarder();
    forward({ location: 0, code: "ControlLeft", key: "Control", type: "keydown", timeStamp: 0 });
    expect(
      forward({ location: 0, code: "AltRight", key: "AltGraph", type: "keydown", timeStamp: 500 }),
    ).toEqual([{ key: "RightAlt", pressed: true }]);
  });
});
