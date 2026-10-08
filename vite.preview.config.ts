import { fileURLToPath } from "node:url";

import { defineConfig, mergeConfig } from "vite";

import base from "./vite.config";

// Browser preview of the real UI against the test FakeBackend (`bun run preview:ui`), for
// looking at a page's design without the Rust side. The three Tauri API modules resolve to the
// stand-in in src/preview/tauri.ts.
const shim = fileURLToPath(new URL("./src/preview/tauri.ts", import.meta.url));

export default mergeConfig(
  base,
  defineConfig({
    resolve: {
      alias: [{ find: /^@tauri-apps\/api\/(core|event|window)$/, replacement: shim }],
    },
    server: { port: 1430, strictPort: true },
  }),
);
