import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AudioInfo, AudioLog } from "../../types";

type LogStore = {
  info: AudioInfo | null;
  log: AudioLog | null;
  logging: boolean;
  error: string;
  mount: () => () => void;
  setLogging: (logging: boolean) => void;
  requestClose: () => void;
};

let loggingTransition: Promise<void> = Promise.resolve();

export const useLogStore = create<LogStore>()((set, get) => {
  let logUnlisten: UnlistenFn | undefined;
  let logGeneration = 0;

  const stopLogging = () => {
    const generation = ++logGeneration;
    logUnlisten?.();
    logUnlisten = undefined;
    loggingTransition = loggingTransition
      .then(async () => {
        if (generation !== logGeneration) {
          return undefined;
        }
        await invoke<AudioLog | null>("set_logging", { logging: false });
        if (generation === logGeneration) {
          set({ log: null });
        }
        return undefined;
      })
      .catch((failure: unknown) => {
        if (generation === logGeneration) {
          set({ error: String(failure) });
        }
        return undefined;
      });
  };

  const startLogging = () => {
    const generation = ++logGeneration;
    let disposed = false;
    let receivedEvent = false;
    loggingTransition = loggingTransition
      .then(async () => {
        if (generation !== logGeneration) {
          return undefined;
        }
        const remove = await listen<AudioLog>("audio-log", ({ payload }) => {
          if (!disposed && generation === logGeneration) {
            receivedEvent = true;
            set({ log: payload });
          }
        });
        if (disposed || generation !== logGeneration) {
          remove();
          return undefined;
        }
        logUnlisten = remove;
        const log = await invoke<AudioLog | null>("set_logging", { logging: true });
        if (!disposed && generation === logGeneration && !receivedEvent && log) {
          set({ log });
        }
        return undefined;
      })
      .catch((failure: unknown) => {
        if (generation === logGeneration) {
          set({ error: String(failure) });
        }
        return undefined;
      });

    return () => {
      disposed = true;
    };
  };

  return {
    info: null,
    log: null,
    logging: false,
    error: "",
    mount: () => {
      let disposed = false;
      let infoSeen = false;
      set({ info: null, error: "" });

      const unlistenInfo = listen<AudioInfo>("log-info", ({ payload }) => {
        if (!disposed) {
          infoSeen = true;
          set({ info: payload });
        }
      });
      void unlistenInfo.catch((failure: unknown) => {
        if (!disposed) {
          set({ error: String(failure) });
        }
      });

      const unlistenClose = listen("request-close", () => {
        if (!disposed) {
          get().requestClose();
        }
      });
      void unlistenClose.catch((failure: unknown) => {
        if (!disposed) {
          set({ error: String(failure) });
        }
      });

      void invoke<AudioInfo>("log_view")
        .then((info) => {
          if (!disposed && !infoSeen) {
            set({ info });
          }
          return undefined;
        })
        .catch((failure: unknown) => {
          if (!disposed) {
            set({ error: String(failure) });
          }
          return undefined;
        });

      return () => {
        disposed = true;
        infoSeen = false;
        if (get().logging) {
          stopLogging();
          set({ logging: false });
        }
        void unlistenInfo.then(
          (remove) => remove(),
          () => {},
        );
        void unlistenClose.then(
          (remove) => remove(),
          () => {},
        );
      };
    },
    setLogging: (logging) => {
      if (logging === get().logging) {
        return;
      }
      set({ logging, error: "" });
      if (logging) {
        startLogging();
      } else {
        stopLogging();
      }
    },
    requestClose: () => {
      void invoke("close_log").catch((failure: unknown) => set({ error: String(failure) }));
    },
  };
});
