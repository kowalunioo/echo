/**
 * Regenerates every app icon from the brand artwork in `branding/`.
 *
 *   bun run icons
 *
 * - Large sizes (128 px and up, Store logos, icon.png) come from `tauri icon` rendering
 *   `branding/echo-icon.svg`.
 * - Small sizes are drawn on their own integer pixel grid instead of being downscaled, so the
 *   bars stay crisp: 16 px uses the hand-tuned `branding/echo-icon-16.svg`; 24, 32 and 48 px are
 *   generated below. `icon.ico` is then rebuilt from 16/24/32/48/64/256 px PNGs.
 *
 * Change the artwork in `branding/` and rerun; the in-app logo (src/components/icons.tsx) draws
 * the same three bars with theme colours and needs no rebuild.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const OUT = join("src-tauri", "icons");
const BRAND_ICON = join("branding", "echo-icon.svg");
const BRAND_ICON_16 = join("branding", "echo-icon-16.svg");

const TILE = "#0A0A0A";
const INK = "#FFFFFF";
const LAVENDER = "#A99BF7";

/** Bars on an integer grid: top/bottom from x0 to x1, the middle one from x0 to xMid. */
interface Grid {
  size: number;
  tileRadius: number;
  barHeight: number;
  barTops: [number, number, number];
  x0: number;
  x1: number;
  xMid: number;
}

const GRIDS: Grid[] = [
  { size: 24, tileRadius: 5.25, barHeight: 4, barTops: [4, 10, 16], x0: 6, x1: 18, xMid: 14 },
  { size: 32, tileRadius: 7, barHeight: 4, barTops: [7, 14, 21], x0: 8, x1: 24, xMid: 19 },
  { size: 48, tileRadius: 10.5, barHeight: 6, barTops: [11, 21, 31], x0: 12, x1: 36, xMid: 28 },
];

function gridSvg(g: Grid): string {
  const rim = g.size / 64; // half of the rim width, scaled like the 32-unit artwork (0.5 at 32)
  const bar = (y: number, x1: number, fill: string) =>
    `<rect x="${g.x0}" y="${y}" width="${x1 - g.x0}" height="${g.barHeight}" rx="${g.barHeight / 2}" fill="${fill}"/>`;
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${g.size} ${g.size}" width="${g.size}" height="${g.size}">`,
    `<rect width="${g.size}" height="${g.size}" rx="${g.tileRadius}" fill="${TILE}"/>`,
    `<rect x="${rim / 2}" y="${rim / 2}" width="${g.size - rim}" height="${g.size - rim}" rx="${g.tileRadius - rim / 2}" fill="none" stroke="${INK}" stroke-opacity="0.12" stroke-width="${rim}"/>`,
    bar(g.barTops[0], g.x1, INK),
    bar(g.barTops[1], g.xMid, LAVENDER),
    bar(g.barTops[2], g.x1, INK),
    `</svg>`,
  ].join("");
}

function tauriIcon(args: string[]) {
  const result = spawnSync("bunx", ["tauri", "icon", ...args], {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

/** Renders `svg` to `<dir>/<size>x<size>.png` with tauri's own renderer. */
function renderPng(svgPath: string, size: number, dir: string): Buffer {
  tauriIcon([svgPath, "--png", String(size), "--output", dir]);
  return readFileSync(join(dir, `${size}x${size}.png`));
}

/** An .ico file with PNG-compressed images (supported since Windows Vista). */
function buildIco(images: { size: number; png: Buffer }[]): Buffer {
  const header = Buffer.alloc(6 + 16 * images.length);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(images.length, 4);
  let offset = header.length;
  images.forEach(({ size, png }, i) => {
    const entry = 6 + 16 * i;
    header.writeUInt8(size >= 256 ? 0 : size, entry); // width (0 means 256)
    header.writeUInt8(size >= 256 ? 0 : size, entry + 1); // height
    header.writeUInt8(0, entry + 2); // palette colours
    header.writeUInt8(0, entry + 3); // reserved
    header.writeUInt16LE(1, entry + 4); // colour planes
    header.writeUInt16LE(32, entry + 6); // bits per pixel
    header.writeUInt32LE(png.length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += png.length;
  });
  return Buffer.concat([header, ...images.map((i) => i.png)]);
}

// 1. Everything tauri generates, from the full-size artwork.
tauriIcon([BRAND_ICON, "--output", OUT]);
for (const unused of ["android", "ios", "icon.icns"]) {
  // Echo is Windows-only for now.
  rmSync(join(OUT, unused), { recursive: true, force: true });
}

// 2. Pixel-snapped small sizes.
const work = mkdtempSync(join(tmpdir(), "echo-icons-"));
try {
  const small = new Map<number, Buffer>();
  small.set(16, renderPng(BRAND_ICON_16, 16, work));
  for (const grid of GRIDS) {
    const svgPath = join(work, `grid-${grid.size}.svg`);
    writeFileSync(svgPath, gridSvg(grid));
    small.set(grid.size, renderPng(svgPath, grid.size, work));
  }
  const png64 = renderPng(BRAND_ICON, 64, work);
  const png256 = renderPng(BRAND_ICON, 256, work);

  writeFileSync(join(OUT, "32x32.png"), small.get(32) ?? Buffer.alloc(0));
  writeFileSync(
    join(OUT, "icon.ico"),
    buildIco([
      ...[16, 24, 32, 48].map((size) => ({ size, png: small.get(size) ?? Buffer.alloc(0) })),
      { size: 64, png: png64 },
      { size: 256, png: png256 },
    ]),
  );
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log("Icons regenerated in", OUT);
