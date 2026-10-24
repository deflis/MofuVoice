import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import { limiterDefaults, type LimiterValues } from "./schema";
type LimiterState = LimiterValues & {
  setEnabled: (value: boolean) => void;
  setCeilingDb: (value: number) => void;
  setReleaseMs: (value: number) => void;
};
export type LimiterSlice = { limiter: LimiterState };
export const createLimiterSlice: StateCreator<SettingsStore, [], [], LimiterSlice> = (set) => ({
  limiter: {
    ...limiterDefaults,
    setEnabled: (value) => set((state) => ({ limiter: { ...state.limiter, enabled: value } })),
    setCeilingDb: (value) => set((state) => ({ limiter: { ...state.limiter, ceilingDb: value } })),
    setReleaseMs: (value) => set((state) => ({ limiter: { ...state.limiter, releaseMs: value } })),
  },
});
