import { create } from "zustand";
import type { AudioLog } from "./types";
import { subscribeAudioLog } from "./audioLogSubscription";

type AudioLogStore = {
  log: AudioLog | null;
  error: string;
  connect: () => () => void;
};

export const useAudioLogStore = create<AudioLogStore>()((set) => {
  let viewers = 0;
  let disconnect: (() => void) | undefined;
  let generation = 0;
  return {
    log: null,
    error: "",
    connect: () => {
      viewers += 1;
      if (viewers === 1) {
        const current = ++generation;
        set({ log: null, error: "" });
        disconnect = subscribeAudioLog(
          (log) => {
            if (current === generation && viewers > 0) {
              set({ log });
            }
          },
          (failure) => {
            if (current === generation && viewers > 0) {
              set({ error: String(failure) });
            }
          },
        );
      }
      let disposed = false;
      return () => {
        if (disposed) {
          return;
        }
        disposed = true;
        viewers -= 1;
        if (viewers === 0) {
          generation += 1;
          disconnect?.();
          disconnect = undefined;
          set({ log: null, error: "" });
        }
      };
    },
  };
});
