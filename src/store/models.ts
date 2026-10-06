import { create } from "zustand";

import { type ModelId, type ModelsState, commands, events } from "../bindings";

/**
 * The frontend's copy of the Models (models.md). The backend owns them: downloads, activation
 * and deletion are commands, and every change comes back as a `modelsChanged` event — including
 * download progress, several times a second.
 */
interface ModelsActions {
  /** Fetches the Models and starts following changes. Safe to call more than once. */
  load: () => Promise<void>;
  download: (model: ModelId) => Promise<void>;
  cancel: (model: ModelId) => Promise<void>;
  /** Makes a downloaded Model active. Loading happens in the background. */
  activate: (model: ModelId) => Promise<void>;
  /** Deletes a downloaded Model or a paused download. */
  remove: (model: ModelId) => Promise<void>;
}

interface ModelsStore extends ModelsActions {
  state: ModelsState | null;
  /** The latest refusal of a command, e.g. switching during a Dictation. */
  refusal: string | null;
}

let stopListening: (() => void) | undefined;

export const useModels = create<ModelsStore>()((set) => {
  const refreshAfter = async (result: { status: "ok" } | { status: "error"; error: string }) => {
    if (result.status === "error") {
      console.error("Model command refused", result.error);
      set({ refusal: result.error, state: await commands.getModels() });
    } else {
      set({ refusal: null });
    }
  };

  return {
    state: null,
    refusal: null,

    load: async () => {
      if (!stopListening) {
        const unlisten = events.modelsChanged.listen((event) => {
          set({ state: event.payload });
        });
        stopListening = () => {
          void unlisten.then((stop) => {
            stop();
          });
        };
      }
      try {
        set({ state: await commands.getModels() });
      } catch (error) {
        console.error("get_models failed", error);
      }
    },

    download: async (model) => {
      await commands.downloadModel(model);
    },

    cancel: async (model) => {
      await commands.cancelModelDownload(model);
    },

    activate: async (model) => {
      await refreshAfter(await commands.activateModel(model));
    },

    remove: async (model) => {
      await refreshAfter(await commands.deleteModel(model));
    },
  };
});

/** Stops following Model changes; for tests that start each case from scratch. */
export function resetModelsStore() {
  stopListening?.();
  stopListening = undefined;
  useModels.setState({ state: null, refusal: null });
}
