import { create } from "zustand";
import type { AudioInfo, AudioLog } from "../../types";

type LogCache = {
  info: AudioInfo | null;
  log: AudioLog | null;
  logging: boolean;
  error: string;
  setInfo: (info: AudioInfo) => void;
  setLog: (log: AudioLog | null) => void;
  setLogging: (logging: boolean) => void;
  setError: (error: string) => void;
  clear: () => void;
};
export const useLogStore = create<LogCache>()((set) => ({
  info: null,
  log: null,
  logging: false,
  error: "",
  setInfo: (info) => set({ info }),
  setLog: (log) => set({ log }),
  setLogging: (logging) => set({ logging }),
  setError: (error) => set({ error }),
  clear: () => set({ info: null, log: null, logging: false, error: "" }),
}));
