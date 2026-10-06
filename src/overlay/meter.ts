/**
 * The Overlay's level meter (overlay.md rule 3): nine bars that follow the input level, ~30
 * frames a second, smoothed so they rise quickly and fall gently.
 */

export const BAR_COUNT = 9;

/** How far each bar reaches at full level: the centre moves most, so the shape stays calm. */
const REACH = [0.42, 0.58, 0.76, 0.9, 1, 0.9, 0.76, 0.58, 0.42];
/** The height of a silent bar, as a fraction of the full height. */
export const BAR_FLOOR = 0.12;
/** Share of the gap to the target closed per frame when rising and when falling. */
const ATTACK = 0.6;
const RELEASE = 0.2;

export const SILENT_BARS: readonly number[] = REACH.map(() => BAR_FLOOR);

/** The bar heights (0–1) one frame on, given the previous heights and the input level (0–1). */
export function nextBars(previous: readonly number[], level: number): number[] {
  const clamped = Math.min(1, Math.max(0, level));
  return REACH.map((reach, i) => {
    const target = BAR_FLOOR + (1 - BAR_FLOOR) * clamped * reach;
    const before = previous[i] ?? BAR_FLOOR;
    const rate = target > before ? ATTACK : RELEASE;
    return before + (target - before) * rate;
  });
}

/** The elapsed-time counter: m:ss. */
export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes)}:${String(seconds % 60).padStart(2, "0")}`;
}
