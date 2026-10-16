import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { LevelValues } from "./schema";

type LevelState = LevelValues & {
  setEnabled: (value: LevelValues["enabled"]) => void;
  setCompressorEnabled: (value: LevelValues["compressorEnabled"]) => void;
  setTargetDb: (value: LevelValues["targetDb"]) => void;
  setMaxGainDb: (value: LevelValues["maxGainDb"]) => void;
  setFloorDb: (value: LevelValues["floorDb"]) => void;
  setResponseMs: (value: LevelValues["responseMs"]) => void;
  setThresholdDb: (value: LevelValues["thresholdDb"]) => void;
  setRatio: (value: LevelValues["ratio"]) => void;
  setAttackMs: (value: LevelValues["attackMs"]) => void;
  setReleaseMs: (value: LevelValues["releaseMs"]) => void;
};
export type LevelSlice = { level: LevelState };
export const createLevelSlice: StateCreator<SettingsStore, [], [], LevelSlice> = (set) => ({
  level: {
    enabled: false,
    compressorEnabled: false,
    targetDb: -20,
    maxGainDb: 9,
    floorDb: -50,
    responseMs: 1500,
    thresholdDb: -12,
    ratio: 3,
    attackMs: 5,
    releaseMs: 150,
    setEnabled: (value) => set((state) => ({ level: { ...state.level, enabled: value } })),
    setCompressorEnabled: (value) =>
      set((state) => ({ level: { ...state.level, compressorEnabled: value } })),
    setTargetDb: (value) => set((state) => ({ level: { ...state.level, targetDb: value } })),
    setMaxGainDb: (value) => set((state) => ({ level: { ...state.level, maxGainDb: value } })),
    setFloorDb: (value) => set((state) => ({ level: { ...state.level, floorDb: value } })),
    setResponseMs: (value) => set((state) => ({ level: { ...state.level, responseMs: value } })),
    setThresholdDb: (value) => set((state) => ({ level: { ...state.level, thresholdDb: value } })),
    setRatio: (value) => set((state) => ({ level: { ...state.level, ratio: value } })),
    setAttackMs: (value) => set((state) => ({ level: { ...state.level, attackMs: value } })),
    setReleaseMs: (value) => set((state) => ({ level: { ...state.level, releaseMs: value } })),
  },
});
