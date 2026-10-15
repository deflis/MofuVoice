import { logApi, type LogApi } from "./api";
import { create } from "zustand";
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

export function createLogStore(api: LogApi = logApi) {
  return create<LogStore>()((set, get) => {
    let lifetime = 0;
    let disconnectCurrent: (() => void) | undefined;
    let loggingTransition: Promise<void> = Promise.resolve();
    let logUnlisten: (() => void) | undefined;
    let logGeneration = 0;

    const stopLogging = () => {
      const generation = ++logGeneration;
      const mounted = lifetime;
      set({ log: null });
      logUnlisten?.();
      logUnlisten = undefined;
      loggingTransition = loggingTransition
        .then(async () => {
          // Always disable the old backend subscription before a queued activation.
          await api.setLogging(false);
          if (generation === logGeneration) {
            set({ log: null });
          }
          return undefined;
        })
        .catch((failure: unknown) => {
          if (generation === logGeneration && mounted === lifetime) {
            set({ error: String(failure) });
          }
          return undefined;
        });
    };

    const startLogging = () => {
      const generation = ++logGeneration;
      let receivedEvent = false;
      loggingTransition = loggingTransition
        .then(async () => {
          if (generation !== logGeneration) {
            return undefined;
          }
          const remove = await api.onLog((payload) => {
            if (generation === logGeneration) {
              receivedEvent = true;
              set({ log: payload });
            }
          });
          if (generation !== logGeneration) {
            remove();
            return undefined;
          }
          logUnlisten = remove;
          const log = await api.setLogging(true);
          if (generation === logGeneration && !receivedEvent && log) {
            set({ log });
          }
          return undefined;
        })
        .catch(async (failure: unknown) => {
          if (generation === logGeneration) {
            logGeneration += 1;
            logUnlisten?.();
            logUnlisten = undefined;
            set({ logging: false, log: null, error: String(failure) });
            // An activation may have reached the backend before its response failed.
            await api.setLogging(false).catch(() => {});
          }
          return undefined;
        });
    };

    return {
      info: null,
      log: null,
      logging: false,
      error: "",
      mount: () => {
        disconnectCurrent?.();
        const mounted = ++lifetime;
        let disposed = false;
        let infoSeen = false;
        set({ info: null, error: "" });

        const unlistenInfo = api.onInfo((payload) => {
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

        const unlistenClose = api.onClose(() => {
          if (!disposed) {
            get().requestClose();
          }
        });
        void unlistenClose.catch((failure: unknown) => {
          if (!disposed) {
            set({ error: String(failure) });
          }
        });

        void api
          .getView()
          .then((info) => {
            if (!disposed && !infoSeen) {
              set({ info });
            }
            return undefined;
          })
          .catch((failure: unknown) => {
            if (!disposed && !infoSeen) {
              set({ error: String(failure) });
            }
            return undefined;
          });

        disconnectCurrent = () => {
          if (disposed) {
            return;
          }
          disposed = true;
          if (lifetime === mounted) {
            lifetime += 1;
          }
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
        return disconnectCurrent;
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
        const generation = lifetime;
        void api.hide().catch((failure: unknown) => {
          if (generation === lifetime) {
            set({ error: String(failure) });
          }
        });
      },
    };
  });
}

export const useLogStore = createLogStore();
