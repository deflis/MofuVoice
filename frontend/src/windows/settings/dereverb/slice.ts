import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import { dereverbDefaults, type DereverbValues } from "./schema";
type DereverbState = DereverbValues & {
  setEnabled: (value: boolean) => void;
  setAmount: (value: number) => void;
  setDecayMs: (value: number) => void;
};
export type DereverbSlice = { dereverb: DereverbState };
export const createDereverbSlice: StateCreator<SettingsStore, [], [], DereverbSlice> = (set) => ({
  dereverb: {
    ...dereverbDefaults,
    setEnabled: (value) => set((state) => ({ dereverb: { ...state.dereverb, enabled: value } })),
    setAmount: (value) => set((state) => ({ dereverb: { ...state.dereverb, amount: value } })),
    setDecayMs: (value) => set((state) => ({ dereverb: { ...state.dereverb, decayMs: value } })),
  },
});
