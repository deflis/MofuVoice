import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { EqValues } from "./schema";
import { copyEqBands } from "./values";

type EqState = EqValues & {
  setEnabled: (value: EqValues["enabled"]) => void;
  setMode: (value: EqValues["mode"]) => void;
  setBands: (value: EqValues["bands"]) => void;
  setOutputDb: (value: EqValues["outputDb"]) => void;
  setBandFrequency: (index: number, value: number) => void;
  setBandGainDb: (index: number, value: number) => void;
  setBandQ: (index: number, value: number) => void;
  reset: () => void;
};
export type EqSlice = { eq: EqState };
export const createEqSlice: StateCreator<SettingsStore, [], [], EqSlice> = (set) => ({
  eq: {
    enabled: false,
    mode: "simple",
    bands: [
      { frequency: 250, gainDb: 0, q: 0.7 },
      { frequency: 3000, gainDb: 0, q: 0.8 },
      { frequency: 8000, gainDb: 0, q: 0.7 },
    ],
    outputDb: 0,
    setEnabled: (value) => set((state) => ({ eq: { ...state.eq, enabled: value } })),
    setMode: (value) => set((state) => ({ eq: { ...state.eq, mode: value } })),
    setBands: (value) => set((state) => ({ eq: { ...state.eq, bands: copyEqBands(value) } })),
    setOutputDb: (value) => set((state) => ({ eq: { ...state.eq, outputDb: value } })),
    setBandFrequency: (index, value) =>
      set((state) => {
        const bands = copyEqBands(state.eq.bands);
        bands[index].frequency = value;
        return { eq: { ...state.eq, bands } };
      }),
    setBandGainDb: (index, value) =>
      set((state) => {
        const bands = copyEqBands(state.eq.bands);
        bands[index].gainDb = value;
        return { eq: { ...state.eq, bands } };
      }),
    setBandQ: (index, value) =>
      set((state) => {
        const bands = copyEqBands(state.eq.bands);
        bands[index].q = value;
        return { eq: { ...state.eq, bands } };
      }),
    reset: () =>
      set((state) => ({
        eq: {
          ...state.eq,
          bands: state.eq.bands.map((band) => ({ ...band, gainDb: 0 })),
          outputDb: 0,
        },
      })),
  },
});
