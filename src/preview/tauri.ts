/**
 * Stand-in for the Tauri API modules in the browser preview (`bun run preview:ui`): the design
 * loop renders the real React app against the test FakeBackend, with one Model ready, so a page
 * can be looked at in a browser without the Rust side. Wired through `vite.preview.config.ts`.
 */
import { backend } from "../test/backend";

const [whisper] = backend.models.models;
if (whisper) {
  backend.models = {
    ...backend.models,
    models: backend.models.models.map((m) =>
      m.id === whisper.id ? { ...m, downloaded: true, download: { state: "idle" } } : m,
    ),
    active: whisper.id,
    activeState: "ready",
  };
  backend.settings = { ...backend.settings, activeModel: whisper.id };
}

// The design loop drives states from the browser console: `echoBackend.changeDictation(...)`.
Object.assign(window, { echoBackend: backend });

export const invoke = backend.invoke;
export const listen = backend.listen;

export function getCurrentWindow() {
  return {
    label: new URLSearchParams(location.search).get("window") ?? "main",
    setTitle: () => Promise.resolve(),
    minimize: () => Promise.resolve(),
    toggleMaximize: () => Promise.resolve(),
    close: () => Promise.resolve(),
  };
}
