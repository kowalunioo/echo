import type { AppInfo, Settings, SettingsPatch } from "../bindings";

/**
 * An in-memory stand-in for the Rust side, used by every frontend test through the Tauri API
 * mocks in `setup.ts`. It answers the generated commands from `bindings.ts` and delivers events,
 * so tests exercise the real binding code. Later slices add their commands to `handlers`.
 */
export const DEFAULT_SETTINGS: Settings = {
  uiLanguage: "en",
  onboardingWelcomeDone: true,
  onboardingCompleted: true,
};

type Handler = (args: Record<string, unknown>) => unknown;
type EventCallback = (event: { event: string; id: number; payload: unknown }) => void;

export class FakeBackend {
  settings: Settings = { ...DEFAULT_SETTINGS };
  appInfo: AppInfo = { version: "0.1.0", systemLocale: "en-US" };
  /** Every command invoked, in order, with its arguments. */
  calls: { command: string; args: Record<string, unknown> }[] = [];
  /** Commands that never answer (to test loading states). */
  hanging = new Set<string>();
  /** Commands that fail, with the error value Tauri rejects with (a string for `Result<_, String>`). */
  failing = new Map<string, unknown>();
  private listeners = new Map<string, Set<EventCallback>>();

  handlers: Record<string, Handler> = {
    app_info: () => this.appInfo,
    get_settings: () => this.settings,
    update_settings: (args) => {
      const patch = args.patch as SettingsPatch;
      const defined = Object.fromEntries(
        Object.entries(patch).filter(([, value]) => value !== null),
      );
      this.changeSettings(defined);
      return this.settings;
    },
    reset_setting: (args) => {
      const key = args.key as keyof Settings;
      this.changeSettings({ [key]: DEFAULT_SETTINGS[key] });
      return this.settings;
    },
    open_log_folder: () => null,
    open_microphone_privacy_settings: () => null,
  };

  /** Changes settings as the backend would (e.g. from the tray) and tells every listener. */
  changeSettings(patch: Partial<Settings>) {
    this.settings = { ...this.settings, ...patch };
    this.emit("settings-changed", this.settings);
  }

  emit(event: string, payload: unknown) {
    for (const callback of this.listeners.get(event) ?? []) {
      callback({ event, id: 0, payload });
    }
  }

  invoke = (command: string, args: Record<string, unknown> = {}): Promise<unknown> => {
    this.calls.push({ command, args });
    if (this.hanging.has(command)) return new Promise(() => undefined);
    if (this.failing.has(command)) {
      // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- Tauri rejects with plain values
      return Promise.reject(this.failing.get(command));
    }
    const handler = this.handlers[command];
    if (!handler) return Promise.reject(new Error(`FakeBackend: unexpected command ${command}`));
    return Promise.resolve(structuredClone(handler(args)));
  };

  listen = (event: string, callback: EventCallback): Promise<() => void> => {
    const set = this.listeners.get(event) ?? new Set();
    set.add(callback);
    this.listeners.set(event, set);
    return Promise.resolve(() => set.delete(callback));
  };

  commandsCalled(command: string) {
    return this.calls.filter((call) => call.command === command);
  }
}

export let backend = new FakeBackend();

export function resetBackend() {
  backend = new FakeBackend();
  return backend;
}
