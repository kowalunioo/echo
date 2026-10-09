/**
 * The Overlay's level meter (overlay.md rule 3): thirteen bars, ~30 frames a second. Each level
 * starts at the centre and travels outward to both edges, fading a little as it goes, so every
 * syllable sends an echo out from the middle; each bar rises quickly and falls gently.
 */

export const BAR_COUNT = 13;

/** The height of a silent bar, as a fraction of the full height. */
export const BAR_FLOOR = 0.12;
/** Share of the gap to the target closed per frame when rising and when falling. */
const ATTACK = 0.6;
const RELEASE = 0.2;
/** The echo moves one bar outward every this many frames (~15 bars a second). */
const FRAMES_PER_STEP = 2;
/** How much of a level is left one bar further out. */
const FADE = 0.88;
/** Levels from the centre (index 0) out to an edge. */
const SIDE = (BAR_COUNT + 1) / 2;

export interface Meter {
  /** Bar heights (0–1), left to right. */
  bars: readonly number[];
  /** Recent levels, newest at the centre (index 0) and oldest at the edges. */
  echo: readonly number[];
  frame: number;
}

export const SILENT_METER: Meter = {
  bars: Array.from({ length: BAR_COUNT }, () => BAR_FLOOR),
  echo: Array.from({ length: SIDE }, () => 0),
  frame: 0,
};

/** The meter one frame on, given the input level (0–1). */
export function nextMeter(previous: Meter, level: number): Meter {
  const clamped = Math.min(1, Math.max(0, level));
  // The centre always shows the live level; on a step the older levels move one bar out.
  const step = previous.frame % FRAMES_PER_STEP === 0;
  const echo = [clamped, ...previous.echo.slice(step ? 0 : 1, step ? SIDE - 1 : SIDE)];
  const bars = previous.bars.map((before, i) => {
    const distance = Math.abs(i - (BAR_COUNT - 1) / 2);
    const target = BAR_FLOOR + (1 - BAR_FLOOR) * (echo[distance] ?? 0) * FADE ** distance;
    const rate = target > before ? ATTACK : RELEASE;
    return before + (target - before) * rate;
  });
  return { bars, echo, frame: previous.frame + 1 };
}

/** The elapsed-time counter: m:ss. */
export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes)}:${String(seconds % 60).padStart(2, "0")}`;
}
