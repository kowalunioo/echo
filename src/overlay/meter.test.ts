import { describe, expect, it } from "vitest";

import { BAR_COUNT, BAR_FLOOR, SILENT_METER, type Meter, formatElapsed, nextMeter } from "./meter";

const CENTRE = (BAR_COUNT - 1) / 2;

/** The meter after these levels, one per frame, from silence. */
function after(levels: readonly number[]): Meter {
  return levels.reduce(nextMeter, SILENT_METER);
}

describe("level meter", () => {
  it("has thirteen bars that rest at the floor in silence", () => {
    expect(BAR_COUNT).toBe(13);
    expect(SILENT_METER.bars).toHaveLength(13);
    expect(after([0, 0, 0]).bars.every((bar) => bar === BAR_FLOOR)).toBe(true);
  });

  it("answers a sound at the centre first", () => {
    const { bars } = after([1]);
    expect(bars[CENTRE]).toBeGreaterThan(0.5);
    expect(bars[0]).toBe(BAR_FLOOR);
    expect(bars[BAR_COUNT - 1]).toBe(BAR_FLOOR);
  });

  it("sends each sound outward to both edges, fading as it goes", () => {
    let meter = SILENT_METER;
    let centrePeak = 0;
    let edgePeak = 0;
    let edgeFirstRose = -1;
    for (let frame = 0; frame < 40; frame++) {
      meter = nextMeter(meter, frame < 2 ? 1 : 0);
      const centre = meter.bars[CENTRE] ?? 0;
      const edge = meter.bars[0] ?? 0;
      centrePeak = Math.max(centrePeak, centre);
      edgePeak = Math.max(edgePeak, edge);
      if (edgeFirstRose < 0 && edge > BAR_FLOOR + 0.05) edgeFirstRose = frame;
    }
    // The ripple reaches the edges later, lower, and the meter settles back to the floor.
    expect(edgeFirstRose).toBeGreaterThan(5);
    expect(edgePeak).toBeGreaterThan(BAR_FLOOR + 0.05);
    expect(edgePeak).toBeLessThan(centrePeak);
    expect(meter.bars.every((bar) => bar < BAR_FLOOR + 0.01)).toBe(true);
  });

  it("stays symmetric around the centre", () => {
    const { bars } = after([0.2, 0.9, 0.4, 1, 0.1, 0.7, 0.3]);
    for (let i = 0; i < BAR_COUNT; i++) {
      expect(bars[i]).toBeCloseTo(bars[BAR_COUNT - 1 - i] ?? 0, 10);
    }
  });

  it("rises quickly and falls gently", () => {
    const up = after([1]).bars[CENTRE] ?? 0;
    const down = after([1, 0]).bars[CENTRE] ?? 0;
    expect(up).toBeGreaterThan(0.5);
    expect(down).toBeLessThan(up);
    expect(down).toBeGreaterThan(0.5 * up);
  });

  it("ignores levels outside 0–1", () => {
    expect(after([-3]).bars).toEqual(after([0]).bars);
    expect(after([7]).bars).toEqual(after([1]).bars);
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
