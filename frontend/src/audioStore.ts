import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Settings, Snapshot } from "./types";

type AudioStore = {
  snapshot: Snapshot;
  pending: boolean;
  closing: boolean;
  error: string;
  connect: (onClose: () => void) => () => void;
  start: (settings: Settings, inspect: boolean) => Promise<void>;
  stop: () => Promise<void>;
  spectrum: () => Promise<void>;
  close: (settings: Settings) => Promise<void>;
};

export const useAudioStore = create<AudioStore>()((set, get) => {
  const run = async (action: () => Promise<void>) => {
    if (get().pending || get().closing) {
      return;
    }
    set({ pending: true, error: "" });
    try {
      await action();
      const previous = get().snapshot;
      const snapshot = await invoke<Snapshot>("audio_status");
      if (get().snapshot === previous) {
        set({ snapshot });
      }
    } catch (failure) {
      set({ error: String(failure) });
    } finally {
      set({ pending: false });
    }
  };

  return {
    snapshot: { busy: false, info: { phase: "ready" } },
    pending: false,
    closing: false,
    error: "",
    connect: (onClose) => {
      let disposed = false;
      const reportError = (failure: unknown) => {
        if (!disposed) {
          set({ error: String(failure) });
        }
      };
      const unlistenStatus = listen<Snapshot>("audio-status", ({ payload }) => {
        if (!disposed) {
          set({ snapshot: payload });
        }
      });
      void unlistenStatus
        .then(async () => {
          if (disposed) {
            return undefined;
          }
          const previous = get().snapshot;
          const snapshot = await invoke<Snapshot>("audio_status");
          if (!disposed && get().snapshot === previous) {
            set({ snapshot });
          }
          return undefined;
        })
        .catch(reportError);
      const unlistenClose = listen("request-close", () => {
        if (!disposed) {
          onClose();
        }
      });
      void unlistenClose.catch(reportError);
      return () => {
        if (disposed) {
          return;
        }
        disposed = true;
        void unlistenStatus.then(
          (remove) => remove(),
          () => {},
        );
        void unlistenClose.then(
          (remove) => remove(),
          () => {},
        );
      };
    },
    start: (settings, inspect) => run(() => invoke<void>("start_audio", { settings, inspect })),
    stop: () => run(() => invoke<void>("stop_audio")),
    spectrum: () => run(() => invoke<void>("open_spectrum")),
    close: async (settings) => {
      if (get().pending || get().closing) {
        return;
      }
      set({ closing: true, error: "" });
      try {
        await invoke<void>("close_application", { settings });
      } catch (failure) {
        set({ closing: false, error: String(failure) });
      }
    },
  };
});
