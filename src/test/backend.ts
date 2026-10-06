import type {
  AppInfo,
  DeviceList,
  DictationStatus,
  HistoryEntry,
  MicrophoneAccess,
  ModelEntry,
  ModelId,
  ModelsState,
  Settings,
  SettingsPatch,
} from "../bindings";

/**
 * An in-memory stand-in for the Rust side, used by every frontend test through the Tauri API
 * mocks in `setup.ts`. It answers the generated commands from `bindings.ts` and delivers events,
 * so tests exercise the real binding code. Later slices add their commands to `handlers`.
 */
export const DEFAULT_SETTINGS: Settings = {
  uiLanguage: "en",
  onboardingWelcomeDone: true,
  onboardingCompleted: true,
  historyLimit: 5,
  microphone: { kind: "default" },
  activeModel: null,
  unloadModelAfter: "never",
  recordShortcut: "Ctrl+Space",
  shortcutMode: "pushToTalk",
  startWithWindows: false,
};

/** The three Models as the backend lists them, none downloaded. */
export function freshModels(): ModelsState {
  const entry = (
    id: ModelId,
    name: string,
    sizeBytes: number,
    languages: number,
    recommended: boolean,
  ): ModelEntry => ({
    id,
    name,
    sizeBytes,
    languages,
    recommended,
    downloaded: false,
    download: { state: "idle" },
  });
  return {
    models: [
      entry("whisperLargeV3Turbo", "Whisper large-v3-turbo", 886_381_760, 100, true),
      entry("parakeetTdt06bV3", "Parakeet TDT 0.6B v3", 739_508_576, 25, false),
      entry("whisperSmall", "Whisper small", 269_751_136, 99, false),
    ],
    active: null,
    activeState: "none",
    activating: null,
    loadFailure: null,
    dictationInProgress: false,
  };
}

type Handler = (args: Record<string, unknown>) => unknown;
type EventCallback = (event: { event: string; id: number; payload: unknown }) => void;

export class FakeBackend {
  settings: Settings = { ...DEFAULT_SETTINGS };
  appInfo: AppInfo = { version: "0.1.0", systemLocale: "en-US" };
  /** The input devices `list_microphones` reports. */
  microphones: DeviceList = {
    devices: ["Microphone (Realtek Audio)"],
    default: "Microphone (Realtek Audio)",
  };
  /** What the Windows microphone privacy check reports. */
  microphoneAccess: MicrophoneAccess = "allowed";
  models: ModelsState = freshModels();
  dictation: DictationStatus = { state: "idle", listening: false, error: null, notices: [] };
  /** Whether Windows "Startup apps" has Echo's sign-in entry turned off. */
  autostartDisabledInWindows = false;
  /** Every command invoked, in order, with its arguments. */
  calls: { command: string; args: Record<string, unknown> }[] = [];
  /** Commands that never answer (to test loading states). */
  hanging = new Set<string>();
  /** Commands that fail, with the error value Tauri rejects with (a string for `Result<_, String>`). */
  failing = new Map<string, unknown>();
  /** History entries, newest first, as the backend's History service keeps them. */
  history: HistoryEntry[] = [];
  /** Texts passed to Re-insert, in order. */
  reinserted: string[] = [];
  private nextHistoryId = 1;
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
    list_history: () => this.history,
    delete_history_entry: (args) => {
      const entry = this.history.find((e) => e.id === args.id) ?? null;
      this.changeHistory(this.history.filter((e) => e.id !== args.id));
      return entry;
    },
    restore_history_entry: (args) => {
      const entry = args.entry as HistoryEntry;
      if (!this.history.some((e) => e.id === entry.id)) {
        this.changeHistory(
          [...this.history, entry].sort((a, b) => b.createdAt - a.createdAt || b.id - a.id),
        );
      }
      return null;
    },
    clear_history: () => {
      this.changeHistory([]);
      return null;
    },
    reinsert_history_entry: (args) => {
      const entry = this.history.find((e) => e.id === args.id);
      if (entry) this.reinserted.push(entry.text);
      return null;
    },
    get_dictation_status: () => this.dictation,
    dictation_window_seen: () => {
      this.changeDictation({ error: null });
      return null;
    },
    dismiss_dictation_notices: () => {
      this.changeDictation({ notices: [] });
      return null;
    },
    open_log_folder: () => null,
    open_microphone_privacy_settings: () => null,
    autostart_status: () => ({
      disabledInWindows: this.settings.startWithWindows && this.autostartDisabledInWindows,
    }),
    open_startup_apps_settings: () => null,
    list_microphones: () => this.microphones,
    microphone_access: () => this.microphoneAccess,
    get_models: () => this.models,
    download_model: () => null,
    cancel_model_download: () => null,
    activate_model: () => null,
    delete_model: () => null,
    // Record Shortcut: accepts any proposal unless the test lists it in `rejectedShortcuts`.
    set_record_shortcut: (args) => {
      const combination = args.combination as string;
      const rejection = this.rejectedShortcuts.get(combination);
      if (rejection !== undefined) throw new RejectedCommand(rejection);
      this.changeSettings({ recordShortcut: combination });
      return this.settings;
    },
    begin_shortcut_capture: () => null,
    end_shortcut_capture: () => null,
    own_window_key: () => null,
  };

  /** Record Shortcut proposals `set_record_shortcut` rejects, with the error it rejects with. */
  rejectedShortcuts = new Map<string, unknown>();

  /** Changes one Model as the backend would and tells every listener. */
  changeModel(id: ModelId, patch: Partial<ModelEntry>) {
    this.changeModels({
      models: this.models.models.map((m) => (m.id === id ? { ...m, ...patch } : m)),
    });
  }

  /** Changes the Models state as the backend would and tells every listener. */
  changeModels(patch: Partial<ModelsState>) {
    this.models = { ...this.models, ...patch };
    this.emit("models-changed", this.models);
  }

  /** Changes settings as the backend would (e.g. from the tray) and tells every listener. */
  changeSettings(patch: Partial<Settings>) {
    this.settings = { ...this.settings, ...patch };
    this.emit("settings-changed", this.settings);
    if (this.history.length > this.settings.historyLimit) this.changeHistory(this.history);
  }

  /** Adds a Transcript as a Dictation would, applying the History limit. */
  addHistoryEntry(text: string, createdAt = Date.now()): HistoryEntry {
    const entry: HistoryEntry = {
      id: this.nextHistoryId++,
      createdAt,
      text,
      model: "whisper-large-v3-turbo",
      language: { kind: "specific", code: "en" },
    };
    this.changeHistory([entry, ...this.history]);
    return entry;
  }

  /** Changes the dictation status as the pipeline would and sends `dictation-status-changed`. */
  changeDictation(patch: Partial<DictationStatus>) {
    this.dictation = { ...this.dictation, ...patch };
    this.emit("dictation-status-changed", this.dictation);
  }

  private changeHistory(entries: HistoryEntry[]) {
    this.history = entries.slice(0, this.settings.historyLimit);
    this.emit("history-changed", this.history);
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
    try {
      return Promise.resolve(structuredClone(handler(args)));
    } catch (error) {
      // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- Tauri rejects with plain values
      if (error instanceof RejectedCommand) return Promise.reject(error.value);
      throw error;
    }
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

/** Thrown by a handler to make its command reject with `value`, as a Rust `Err` does. */
class RejectedCommand extends Error {
  constructor(readonly value: unknown) {
    super("rejected command");
  }
}

export let backend = new FakeBackend();

export function resetBackend() {
  backend = new FakeBackend();
  return backend;
}
