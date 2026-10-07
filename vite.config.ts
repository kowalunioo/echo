/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri expects a fixed dev-server port and must see Rust compile errors, so the screen is not
// cleared. See https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    // Each test file still gets a fresh context, but inside a reused worker instead of a new one,
    // which roughly halves `bun run test`. `isolate: false` was as fast but leaked state between
    // files (TestAudioMarker failed intermittently), so isolation stays on.
    pool: "vmThreads",
    // Only the repo's own tests: local agent worktrees under .claude/ hold full repo copies.
    include: ["src/**/*.test.{ts,tsx}", "scripts/**/*.test.ts"],
  },
});
