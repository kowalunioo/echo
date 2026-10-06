import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";

import { initI18n } from "../i18n";
import { resetModelsStore } from "../store/models";
import { resetRecordShortcutStore } from "../store/recordShortcut";
import { resetSettingsStore } from "../store/settings";
import { backend, resetBackend } from "./backend";

// Every test talks to a fresh in-memory FakeBackend through the real generated bindings.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => backend.invoke(command, args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, callback: Parameters<typeof backend.listen>[1]) =>
    backend.listen(event, callback),
}));

initI18n("en");

beforeEach(() => {
  resetBackend();
  resetSettingsStore();
  resetModelsStore();
  resetRecordShortcutStore();
});

afterEach(() => {
  cleanup();
});
