import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { EqValues } from "./schema";
import { copyEqBands } from "./values";

export type EqState = EqValues & {
  setEnabled: (value: EqValues["enabled"]) => void;
  setMode: (value: EqValues["mode"]) => void;
  setBands: (value: EqValues["bands"]) => void;
  setOutputDb: (value: EqValues["outputDb"]) => void;
  setBandFrequency: (index: number, value: number) => void;
  setBandGainDb: (index: number, value: number) => void;
  setBandQ: (index: number, value: number) => void;
  reset: () => void;
};
export type EqKey = "eq" | "inputEq" | "preConversionEq";
export type EqSlice = Record<EqKey, EqState>;
export const createEqSlice: StateCreator<SettingsStore, [], [], EqSlice> = (set) => {
  const create = (key: EqKey): EqState => ({
    enabled: false,
    mode: "simple",
    bands: [
      { frequency: 250, gainDb: 0, q: 0.7 },
      { frequency: 3000, gainDb: 0, q: 0.8 },
      { frequency: 8000, gainDb: 0, q: 0.7 },
    ],
    outputDb: 0,
    setEnabled: (value) => set((state) => ({ [key]: { ...state[key], enabled: value } })),
    setMode: (value) => set((state) => ({ [key]: { ...state[key], mode: value } })),
    setBands: (value) => set((state) => ({ [key]: { ...state[key], bands: copyEqBands(value) } })),
    setOutputDb: (value) => set((state) => ({ [key]: { ...state[key], outputDb: value } })),
    setBandFrequency: (index, value) =>
      set((state) => {
        const bands = copyEqBands(state[key].bands);
        bands[index].frequency = value;
        return { [key]: { ...state[key], bands } };
      }),
    setBandGainDb: (index, value) =>
      set((state) => {
        const bands = copyEqBands(state[key].bands);
        bands[index].gainDb = value;
        return { [key]: { ...state[key], bands } };
      }),
    setBandQ: (index, value) =>
      set((state) => {
        const bands = copyEqBands(state[key].bands);
        bands[index].q = value;
        return { [key]: { ...state[key], bands } };
      }),
    reset: () =>
      set((state) => ({
        [key]: {
          ...state[key],
          bands: state[key].bands.map((band) => ({ ...band, gainDb: 0 })),
          outputDb: 0,
        },
      })),
  });
  return {
    eq: create("eq"),
    inputEq: create("inputEq"),
    preConversionEq: create("preConversionEq"),
  };
};
