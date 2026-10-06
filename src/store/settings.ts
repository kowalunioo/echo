import { create } from "zustand";

import { type Settings, commands, events } from "../bindings";

/**
 * The frontend's copy of the backend settings (settings-and-first-run.md rules 11–15). The
 * backend owns them: every change goes through `update_settings`, is saved at once, and comes
 * back to every window as a `settingsChanged` event. See docs/settings.md.
 *
 * In components, read and change one setting with `useSetting`:
 *
 *     const [language, setLanguage] = useSetting("uiLanguage");
 */
export type SettingKey = keyof Settings;

export type SettingsState =
  | { status: "loading"; settings: null }
  | { status: "ready"; settings: Settings }
  | { status: "error"; settings: null };

interface SettingsActions {
  /** Fetches the settings and starts following changes made anywhere (backend, other windows). */
  load: () => Promise<void>;
  /** Changes one setting. The UI updates at once; the backend's answer is then authoritative. */
  set: <K extends SettingKey>(key: K, value: Settings[K]) => Promise<void>;
  /** Puts one setting back to its default (the "reset to default" controls, rule 15). */
  reset: (key: SettingKey) => Promise<void>;
}

let stopListening: (() => void) | undefined;

export const useSettings = create<SettingsState & SettingsActions>()((set, get) => ({
  status: "loading",
  settings: null,

  load: async () => {
    if (!stopListening) {
      const unlisten = events.settingsChanged.listen((event) => {
        set({ status: "ready", settings: event.payload });
      });
      stopListening = () => {
        void unlisten.then((stop) => {
          stop();
        });
      };
    }
    try {
      set({ status: "ready", settings: await commands.getSettings() });
    } catch (error) {
      console.error("get_settings failed", error);
      set({ status: "error", settings: null });
    }
  },

  set: async (key, value) => {
    const current = get().settings;
    if (!current) throw new Error(`setting ${key} changed before the settings were loaded`);
    set({ settings: { ...current, [key]: value } });
    const result = await commands.updateSettings({ [key]: value });
    if (result.status === "ok") {
      set({ settings: result.data });
    } else {
      console.error(`could not change setting ${key}`, result.error);
      set({ settings: await commands.getSettings() });
    }
  },

  reset: async (key) => {
    const result = await commands.resetSetting(key);
    if (result.status === "ok") {
      set({ settings: result.data });
    } else {
      console.error(`could not reset setting ${key}`, result.error);
    }
  },
}));

/** Stops following settings changes; for tests that start each case from scratch. */
export function resetSettingsStore() {
  stopListening?.();
  stopListening = undefined;
  useSettings.setState({ status: "loading", settings: null });
}

/**
 * One setting and a function that changes it. Only for components rendered after the settings
 * have loaded (the app shows nothing before that).
 */
export function useSetting<K extends SettingKey>(
  key: K,
): [Settings[K], (value: Settings[K]) => Promise<void>] {
  const value = useSettings((s) => {
    if (!s.settings) throw new Error(`useSetting("${key}") before the settings loaded`);
    return s.settings[key];
  });
  const setSetting = useSettings((s) => s.set);
  return [value, (next) => setSetting(key, next)];
}
