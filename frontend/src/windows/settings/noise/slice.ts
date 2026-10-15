import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { NoiseValues } from "./schema";

type NoiseState = NoiseValues & {
  setRumbleFilter: (value: NoiseValues["rumbleFilter"]) => void;
  setEnabled: (value: NoiseValues["enabled"]) => void;
  setThresholdMode: (value: NoiseValues["thresholdMode"]) => void;
  setThresholdDb: (value: NoiseValues["thresholdDb"]) => void;
  setAttackMs: (value: NoiseValues["attackMs"]) => void;
  setReleaseMs: (value: NoiseValues["releaseMs"]) => void;
};
export type NoiseSlice = { noise: NoiseState };
export const createNoiseSlice: StateCreator<SettingsStore, [], [], NoiseSlice> = (set) => ({
  noise: {
    rumbleFilter: false,
    enabled: false,
    thresholdMode: "auto",
    thresholdDb: -50,
    attackMs: 5,
    releaseMs: 80,
    setRumbleFilter: (value) =>
      set((state) => ({ noise: { ...state.noise, rumbleFilter: value } })),
    setEnabled: (value) => set((state) => ({ noise: { ...state.noise, enabled: value } })),
    setThresholdMode: (value) =>
      set((state) => {
        const thresholdDb =
          value === "auto" &&
          !(
            Number.isFinite(state.noise.thresholdDb) &&
            state.noise.thresholdDb >= -90 &&
            state.noise.thresholdDb <= 0
          )
            ? -50
            : state.noise.thresholdDb;
        return { noise: { ...state.noise, thresholdMode: value, thresholdDb } };
      }),
    setThresholdDb: (value) => set((state) => ({ noise: { ...state.noise, thresholdDb: value } })),
    setAttackMs: (value) => set((state) => ({ noise: { ...state.noise, attackMs: value } })),
    setReleaseMs: (value) => set((state) => ({ noise: { ...state.noise, releaseMs: value } })),
  },
});
