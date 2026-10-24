import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import { reverbDefaults, type ReverbValues } from "./schema";
type ReverbState = ReverbValues & {
  setEnabled: (value: boolean) => void;
  setMix: (value: number) => void;
  setPreDelayMs: (value: number) => void;
  setDecayMs: (value: number) => void;
};
export type ReverbSlice = { reverb: ReverbState };
export const createReverbSlice: StateCreator<SettingsStore, [], [], ReverbSlice> = (set) => ({
  reverb: {
    ...reverbDefaults,
    setEnabled: (value) => set((state) => ({ reverb: { ...state.reverb, enabled: value } })),
    setMix: (value) => set((state) => ({ reverb: { ...state.reverb, mix: value } })),
    setPreDelayMs: (value) => set((state) => ({ reverb: { ...state.reverb, preDelayMs: value } })),
    setDecayMs: (value) => set((state) => ({ reverb: { ...state.reverb, decayMs: value } })),
  },
});
