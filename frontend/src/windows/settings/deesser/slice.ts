import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { DeesserValues } from "./schema";

type DeesserState = DeesserValues & {
  setEnabled: (value: DeesserValues["enabled"]) => void;
  setMode: (value: DeesserValues["mode"]) => void;
  setFrequencyHz: (value: DeesserValues["frequencyHz"]) => void;
  setThresholdDb: (value: DeesserValues["thresholdDb"]) => void;
  setRangeDb: (value: DeesserValues["rangeDb"]) => void;
  setAttackMs: (value: DeesserValues["attackMs"]) => void;
  setReleaseMs: (value: DeesserValues["releaseMs"]) => void;
};
export type DeesserSlice = { deesser: DeesserState };
export const createDeesserSlice: StateCreator<SettingsStore, [], [], DeesserSlice> = (set) => ({
  deesser: {
    enabled: false,
    mode: "simple",
    frequencyHz: 5000,
    thresholdDb: -24,
    rangeDb: 6,
    attackMs: 0.5,
    releaseMs: 60,
    setEnabled: (value) => set((state) => ({ deesser: { ...state.deesser, enabled: value } })),
    setMode: (value) => set((state) => ({ deesser: { ...state.deesser, mode: value } })),
    setFrequencyHz: (value) =>
      set((state) => ({ deesser: { ...state.deesser, frequencyHz: value } })),
    setThresholdDb: (value) =>
      set((state) => ({ deesser: { ...state.deesser, thresholdDb: value } })),
    setRangeDb: (value) => set((state) => ({ deesser: { ...state.deesser, rangeDb: value } })),
    setAttackMs: (value) => set((state) => ({ deesser: { ...state.deesser, attackMs: value } })),
    setReleaseMs: (value) => set((state) => ({ deesser: { ...state.deesser, releaseMs: value } })),
  },
});
