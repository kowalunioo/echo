/**
 * Regenerates the tray icons (`docs/specs/tray.md` rules 2–4b) in `src-tauri/icons/tray/`.
 *
 *   bun run tray-icons
 *
 * The glyph is the logo's three rounded bars without the tile; the middle bar shows the state:
 *
 * - idle: a short bar in the glyph colour (as in the logo);
 * - recording: a full-length lavender bar;
 * - transcribing: three lavender dots;
 * - error: the idle bars with a red badge carrying a white exclamation mark, cut out of the bars
 *   so it reads on any taskbar — a different colour and shape from recording (rule 4b).
 *
 * Every state comes in two variants: `dark` glyphs for a light taskbar and `light` glyphs for a
 * dark one (rule 3). Each pixel size Windows uses for notification-area icons (16 px at 100 %,
 * 20 at 125 %, 24 at 150 %, 32 at 200 %) is drawn on its own integer grid rather than downscaled,
 * then rendered by `tauri icon`, like the app icons in `scripts/icons.ts`.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const OUT = join("src-tauri", "icons", "tray");

/** Glyph colours; `dark` is drawn on a light taskbar, `light` on a dark one. */
const VARIANTS = {
  dark: { ink: "#1C1B21", accent: "#6C5BD3", danger: "#D92D20" },
  light: { ink: "#FFFFFF", accent: "#A99BF7", danger: "#F04438" },
} as const;

const STATES = ["idle", "recording", "transcribing", "error"] as const;
type State = (typeof STATES)[number];

/** One size on its pixel grid. Bars run from x0 to x1 (idle middle bar to xMid). */
interface Grid {
  size: number;
  barHeight: number;
  barTops: [number, number, number];
  x0: number;
  x1: number;
  xMid: number;
  /** Left edges of the three transcribing dots (each barHeight square). */
  dots: [number, number, number];
  /** The error badge: centre, radius, the gap cut around it, and its "!" (stem, then dot). */
  badge: {
    c: number;
    r: number;
    gap: number;
    x: number;
    w: number;
    stem: [number, number];
    dot: [number, number];
  };
}

const GRIDS: Grid[] = [
  {
    size: 16,
    barHeight: 2,
    barTops: [3, 7, 11],
    x0: 1,
    x1: 15,
    xMid: 10,
    dots: [1, 7, 13],
    badge: { c: 11.5, r: 4.5, gap: 1, x: 11, w: 1, stem: [8, 12], dot: [13, 14] },
  },
  {
    size: 20,
    barHeight: 3,
    barTops: [2, 8, 14],
    x0: 2,
    x1: 19,
    xMid: 13,
    dots: [2, 9, 16],
    badge: { c: 15, r: 5, gap: 1, x: 14, w: 2, stem: [11, 15], dot: [16, 18] },
  },
  {
    size: 24,
    barHeight: 4,
    barTops: [3, 10, 17],
    x0: 2,
    x1: 22,
    xMid: 15,
    dots: [2, 10, 18],
    badge: { c: 18, r: 6, gap: 1.5, x: 17, w: 2, stem: [14, 19], dot: [20, 22] },
  },
  {
    size: 32,
    barHeight: 4,
    barTops: [6, 14, 22],
    x0: 3,
    x1: 29,
    xMid: 20,
    dots: [3, 14, 25],
    badge: { c: 24, r: 8, gap: 2, x: 23, w: 2, stem: [19, 26], dot: [27, 29] },
  },
];

function glyphSvg(
  g: Grid,
  state: State,
  colours: (typeof VARIANTS)[keyof typeof VARIANTS],
): string {
  const h = g.barHeight;
  const bar = (y: number, x1: number, fill: string) =>
    `<rect x="${g.x0}" y="${y}" width="${x1 - g.x0}" height="${h}" rx="${h / 2}" fill="${fill}"/>`;
  const middle =
    state === "recording"
      ? bar(g.barTops[1], g.x1, colours.accent)
      : state === "transcribing"
        ? g.dots
            .map(
              (x) =>
                `<rect x="${x}" y="${g.barTops[1]}" width="${h}" height="${h}" rx="${h / 2}" fill="${colours.accent}"/>`,
            )
            .join("")
        : bar(g.barTops[1], g.xMid, colours.ink);
  const bars = [
    bar(g.barTops[0], g.x1, colours.ink),
    middle,
    bar(g.barTops[2], g.x1, colours.ink),
  ].join("");
  const svg = (body: string) =>
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${g.size} ${g.size}" width="${g.size}" height="${g.size}">${body}</svg>`;
  if (state !== "error") return svg(bars);

  const b = g.badge;
  return svg(
    [
      `<defs><mask id="cut"><rect width="${g.size}" height="${g.size}" fill="#fff"/>`,
      `<circle cx="${b.c}" cy="${b.c}" r="${b.r + b.gap}" fill="#000"/></mask></defs>`,
      `<g mask="url(#cut)">${bars}</g>`,
      `<circle cx="${b.c}" cy="${b.c}" r="${b.r}" fill="${colours.danger}"/>`,
      `<rect x="${b.x}" y="${b.stem[0]}" width="${b.w}" height="${b.stem[1] - b.stem[0]}" fill="#FFFFFF"/>`,
      `<rect x="${b.x}" y="${b.dot[0]}" width="${b.w}" height="${b.dot[1] - b.dot[0]}" fill="#FFFFFF"/>`,
    ].join(""),
  );
}

function renderPng(svgPath: string, size: number, dir: string): Buffer {
  const result = spawnSync(
    "bunx",
    ["tauri", "icon", svgPath, "--png", String(size), "--output", dir],
    {
      stdio: ["ignore", "ignore", "inherit"],
      shell: process.platform === "win32",
    },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
  return readFileSync(join(dir, `${size}x${size}.png`));
}

mkdirSync(OUT, { recursive: true });
const work = mkdtempSync(join(tmpdir(), "echo-tray-icons-"));
try {
  for (const [variant, colours] of Object.entries(VARIANTS)) {
    for (const state of STATES) {
      for (const grid of GRIDS) {
        const name = `${state}-${variant}-${grid.size}`;
        const svgPath = join(work, `${name}.svg`);
        writeFileSync(svgPath, glyphSvg(grid, state, colours));
        writeFileSync(join(OUT, `${name}.png`), renderPng(svgPath, grid.size, work));
      }
    }
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log("Tray icons regenerated in", OUT);
