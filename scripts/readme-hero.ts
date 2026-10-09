/**
 * Regenerates the README hero, `docs/assets/hero-light.svg` and `docs/assets/hero-dark.svg`.
 *
 *   bun run readme-hero
 *
 * Each hero shows the logo, the name, a tagline and the Overlay pill while listening. The pill's
 * level meter is the real one: a made-up loop of speech levels is fed through `nextMeter`
 * (src/overlay/meter.ts) and the bar heights it produces are baked into CSS keyframes, so the
 * echo ripples out from the centre exactly as it does in the app. GitHub shows README SVGs
 * through `<img>`, which blocks scripts, external fonts and images, so everything is inline and
 * the motion is CSS only. With reduced motion the meter stands still and the edge does not shine.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { BAR_COUNT, type Meter, SILENT_METER, nextMeter } from "../src/overlay/meter";

const OUT = join("docs", "assets");

/** Colours from the theme tokens in src/styles.css. */
interface Theme {
  bg: string;
  line: string;
  surface: string;
  fg: string;
  muted: string;
  accent: string;
  edge: string;
  shine: string;
  shadow: number;
}

const LIGHT: Theme = {
  bg: "#f7f7f8",
  line: "#e3e3e6",
  surface: "#ffffff",
  fg: "#1f1f22",
  muted: "#65656c",
  accent: "#7c6be0",
  edge: "rgb(0 0 0 / 0.28)",
  shine: "rgb(31 31 34 / 0.4)",
  shadow: 0.1,
};

const DARK: Theme = {
  bg: "#1b1b1d",
  line: "#303034",
  surface: "#222225",
  fg: "#ececee",
  muted: "#9a9aa2",
  accent: "#a99bf7",
  edge: "rgb(255 255 255 / 0.22)",
  shine: "rgb(255 255 255 / 0.55)",
  shadow: 0.45,
};

const TAGLINE =
  "Private, local dictation for Windows. Speak, and your words appear where you type.";

const WIDTH = 880;
const HEIGHT = 300;

/** The pill in its own pixels (256 × 44, as in the app), drawn 1.5× larger. */
const PILL = { width: 256, height: 44, scale: 1.5, top: 196 };
/** Meter frames per second and frames in one loop of the animation. */
const FPS = 30;
const LOOP = 120;
/** The meter's bars: 3 px wide, 4 px apart, 22 px tall at full level. */
const BAR = { width: 3, gap: 4, height: 22 };

/** Speech levels for one loop: words of a few syllables with pauses between them. */
function levels(): number[] {
  const words: [start: number, peaks: number[]][] = [
    [4, [0.55, 0.9, 0.7]],
    [34, [0.8, 0.5]],
    [56, [0.65, 0.95, 0.6, 0.75]],
    [92, [0.7, 0.85]],
  ];
  const frames = Array.from({ length: LOOP }, () => 0.02);
  const syllable = 7;
  for (const [start, peaks] of words) {
    peaks.forEach((peak, n) => {
      for (let f = 0; f < syllable; f++) {
        const at = start + n * syllable + f;
        if (at < LOOP)
          frames[at] = Math.max(frames[at] ?? 0, peak * Math.sin((Math.PI * (f + 0.5)) / syllable));
      }
    });
  }
  return frames;
}

/** Bar heights for every frame of a loop, from the meter's steady state. */
function meterFrames(): number[][] {
  const input = levels();
  let meter: Meter = SILENT_METER;
  const frames: number[][] = [];
  for (let lap = 0; lap < 3; lap++) {
    for (const level of input) {
      meter = nextMeter(meter, level);
      if (lap === 2) frames.push([...meter.bars]);
    }
  }
  return frames;
}

const round = (n: number) => Number(n.toFixed(3)).toString();

/** One keyframe rule per distance from the centre; the meter is symmetric. */
function meterCss(frames: number[][]): { keyframes: string; still: string } {
  const centre = (BAR_COUNT - 1) / 2;
  // With reduced motion the meter holds the loudest frame.
  const loudest = frames.reduce((best, f) =>
    f.reduce((a, b) => a + b) > best.reduce((a, b) => a + b) ? f : best,
  );
  const height = (bars: number[] | undefined, d: number) => round(bars?.[centre + d] ?? 0);
  const keyframes: string[] = [];
  const still: string[] = [];
  for (let d = 0; d <= centre; d++) {
    const stops = frames.map(
      (bars, i) => `${round((i / LOOP) * 100)}%{transform:scaleY(${height(bars, d)})}`,
    );
    stops.push(`100%{transform:scaleY(${height(frames[0], d)})}`);
    keyframes.push(`@keyframes d${String(d)}{${stops.join("")}}`);
    still.push(
      `.d${String(d)}{transform:scaleY(${height(loudest, d)});animation-name:d${String(d)}}`,
    );
  }
  return { keyframes: keyframes.join("\n"), still: still.join("\n") };
}

function hero(theme: Theme, mode: "light" | "dark", frames: number[][]): string {
  const { keyframes, still } = meterCss(frames);
  const pillWidth = PILL.width * PILL.scale;
  const pillLeft = (WIDTH - pillWidth) / 2;
  const r = PILL.height / 2;

  // Inside the pill, as in OverlayApp: 17 px padding, the mark, 12 px gap, the meter, 12 px gap,
  // the 28 px cancel button and 8 px padding.
  const meterLeft = 17 + 14 + 12;
  const meterWidth = PILL.width - meterLeft - 12 - 28 - 8;
  const barsWidth = BAR_COUNT * BAR.width + (BAR_COUNT - 1) * BAR.gap;
  const barsLeft = meterLeft + (meterWidth - barsWidth) / 2;
  const barTop = (PILL.height - BAR.height) / 2;
  const centre = (BAR_COUNT - 1) / 2;
  const bars = Array.from({ length: BAR_COUNT }, (_, i) => {
    const x = barsLeft + i * (BAR.width + BAR.gap);
    const d = Math.abs(i - centre);
    return `<rect class="bar d${String(d)}" x="${round(x)}" y="${round(barTop)}" width="${String(BAR.width)}" height="${String(BAR.height)}" rx="1.5"/>`;
  }).join("\n      ");
  const cancelX = PILL.width - 8 - 14;

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${String(WIDTH)} ${String(HEIGHT)}" width="${String(WIDTH)}" height="${String(HEIGHT)}" role="img" aria-labelledby="title desc">
  <title id="title">Echo</title>
  <desc id="desc">The Echo logo and name, the tagline "${TAGLINE}", and Echo's recording Overlay: a small pill whose level meter ripples out from the centre while you speak.</desc>
  <!-- Generated by scripts/readme-hero.ts (${mode}); edit the script, not this file. -->
  <style>
    text { font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, -apple-system, "Helvetica Neue", Arial, sans-serif; }
    .name { font-size: 64px; font-weight: 650; letter-spacing: -0.02em; fill: ${theme.fg}; }
    .tagline { font-size: 20px; font-weight: 400; fill: ${theme.muted}; }
    .bar { fill: ${theme.accent}; transform-box: fill-box; transform-origin: center;
      animation-duration: ${String(LOOP / FPS)}s; animation-timing-function: linear; animation-iteration-count: infinite; }
    .shine { animation: shine 3.4s linear infinite; }
    @keyframes shine { from { transform: translateX(-${String(PILL.width)}px); } to { transform: translateX(${String(PILL.width)}px); } }
${still}
${keyframes}
    @media (prefers-reduced-motion: reduce) {
      .bar { animation: none; }
      .shine { animation: none; opacity: 0; }
    }
  </style>
  <defs>
    <linearGradient id="band" x1="0" y1="0" x2="1" y2="0.18">
      <stop offset="0.35" stop-color="${theme.shine}" stop-opacity="0"/>
      <stop offset="0.5" stop-color="${theme.shine}"/>
      <stop offset="0.65" stop-color="${theme.shine}" stop-opacity="0"/>
    </linearGradient>
    <mask id="edge">
      <rect x="0.5" y="0.5" width="${String(PILL.width - 1)}" height="${String(PILL.height - 1)}" rx="${String(r - 0.5)}" fill="none" stroke="#fff" stroke-width="1"/>
    </mask>
    <filter id="lift" x="-20%" y="-60%" width="140%" height="220%">
      <feDropShadow dx="0" dy="4" stdDeviation="8" flood-color="#000" flood-opacity="${String(theme.shadow)}"/>
    </filter>
  </defs>

  <rect x="0.5" y="0.5" width="${String(WIDTH - 1)}" height="${String(HEIGHT - 1)}" rx="20" fill="${theme.bg}" stroke="${theme.line}"/>

  <!-- Logo (the three bars, as in the app) and name -->
  <g transform="translate(330 52)">
    <svg viewBox="7 6 18 20" width="54" height="60">
      <rect x="8" y="7" width="16" height="4" rx="2" fill="${theme.fg}"/>
      <rect x="8" y="14" width="11" height="4" rx="2" fill="${theme.accent}"/>
      <rect x="8" y="21" width="16" height="4" rx="2" fill="${theme.fg}"/>
    </svg>
    <text class="name" x="74" y="53">Echo</text>
  </g>

  <text class="tagline" x="${String(WIDTH / 2)}" y="160" text-anchor="middle">${TAGLINE}</text>

  <!-- The Overlay while listening -->
  <g transform="translate(${String(pillLeft)} ${String(PILL.top)}) scale(${String(PILL.scale)})">
    <rect width="${String(PILL.width)}" height="${String(PILL.height)}" rx="${String(r)}" fill="${theme.surface}" filter="url(#lift)"/>
    <g mask="url(#edge)">
      <rect width="${String(PILL.width)}" height="${String(PILL.height)}" fill="${theme.edge}"/>
      <rect class="shine" width="${String(PILL.width)}" height="${String(PILL.height)}" fill="url(#band)"/>
    </g>
    <svg x="17" y="${String((PILL.height - 16) / 2)}" width="14" height="16" viewBox="7 6 18 20">
      <rect x="8" y="7" width="16" height="4" rx="2" fill="${theme.fg}"/>
      <rect x="8" y="14" width="11" height="4" rx="2" fill="${theme.accent}"/>
      <rect x="8" y="21" width="16" height="4" rx="2" fill="${theme.fg}"/>
    </svg>
    <g>
      ${bars}
    </g>
    <path d="M${String(cancelX - 3)} ${String(r - 3)}l6 6M${String(cancelX + 3)} ${String(r - 3)}l-6 6" stroke="${theme.muted}" stroke-width="1.4" stroke-linecap="round"/>
  </g>
</svg>
`;
}

const frames = meterFrames();
mkdirSync(OUT, { recursive: true });
writeFileSync(join(OUT, "hero-light.svg"), hero(LIGHT, "light", frames));
writeFileSync(join(OUT, "hero-dark.svg"), hero(DARK, "dark", frames));
console.log(`Wrote ${join(OUT, "hero-light.svg")} and ${join(OUT, "hero-dark.svg")}`);
