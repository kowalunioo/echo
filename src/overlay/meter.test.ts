import { describe, expect, it } from "vitest";

import { BAR_COUNT, BAR_FLOOR, SILENT_BARS, formatElapsed, nextBars } from "./meter";

describe("level meter", () => {
  it("has nine bars that rest at the floor in silence", () => {
    expect(BAR_COUNT).toBe(9);
    expect(nextBars(SILENT_BARS, 0)).toEqual(SILENT_BARS);
  });

  it("rises quickly and falls gently", () => {
    const up = nextBars(SILENT_BARS, 1);
    const centre = Math.floor(BAR_COUNT / 2);
    expect(up[centre]).toBeGreaterThan(0.5);
    const down = nextBars(up, 0);
    expect(down[centre]).toBeLessThan(up[centre] ?? 0);
    expect(down[centre]).toBeGreaterThan(0.5 * (up[centre] ?? 0));
  });

  it("settles on a shape with the centre tallest", () => {
    let bars = [...SILENT_BARS];
    for (let i = 0; i < 30; i++) bars = nextBars(bars, 1);
    expect(bars[4]).toBeCloseTo(1, 2);
    expect(bars[0]).toBeLessThan(bars[2] ?? 0);
    expect(bars[0]).toBeCloseTo(bars[8] ?? 0, 5);
    expect(Math.min(...bars)).toBeGreaterThan(BAR_FLOOR);
  });

  it("ignores levels outside 0–1", () => {
    expect(nextBars(SILENT_BARS, -3)).toEqual(SILENT_BARS);
    expect(nextBars(SILENT_BARS, 7)).toEqual(nextBars(SILENT_BARS, 1));
  });
});

describe("elapsed time", () => {
  it("is shown as m:ss", () => {
    expect(formatElapsed(0)).toBe("0:00");
    expect(formatElapsed(9_999)).toBe("0:09");
    expect(formatElapsed(65_000)).toBe("1:05");
    expect(formatElapsed(600_000)).toBe("10:00");
  });
});
