import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import { compressorDefaults, type CompressorValues } from "./schema";
type CompressorState = CompressorValues & {
  setEnabled: (value: boolean) => void;
  setThresholdDb: (value: number) => void;
  setRatio: (value: number) => void;
  setAttackMs: (value: number) => void;
  setReleaseMs: (value: number) => void;
};
export type CompressorSlice = { compressor: CompressorState };
export const createCompressorSlice: StateCreator<SettingsStore, [], [], CompressorSlice> = (
  set,
) => ({
  compressor: {
    ...compressorDefaults,
    setEnabled: (value) =>
      set((state) => ({ compressor: { ...state.compressor, enabled: value } })),
    setThresholdDb: (value) =>
      set((state) => ({ compressor: { ...state.compressor, thresholdDb: value } })),
    setRatio: (value) => set((state) => ({ compressor: { ...state.compressor, ratio: value } })),
    setAttackMs: (value) =>
      set((state) => ({ compressor: { ...state.compressor, attackMs: value } })),
    setReleaseMs: (value) =>
      set((state) => ({ compressor: { ...state.compressor, releaseMs: value } })),
  },
});
