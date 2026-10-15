import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { VoiceValues } from "./schema";

type VoiceState = VoiceValues & {
  setPitch: (value: VoiceValues["pitch"]) => void;
  setFormantShift: (value: VoiceValues["formantShift"]) => void;
  setPreserveFormants: (value: VoiceValues["preserveFormants"]) => void;
};
export type VoiceSlice = { voice: VoiceState };
export const createVoiceSlice: StateCreator<SettingsStore, [], [], VoiceSlice> = (set) => ({
  voice: {
    pitch: 0,
    formantShift: 0,
    preserveFormants: true,
    setPitch: (value) => set((state) => ({ voice: { ...state.voice, pitch: value } })),
    setFormantShift: (value) =>
      set((state) => ({ voice: { ...state.voice, formantShift: value } })),
    setPreserveFormants: (value) =>
      set((state) => ({ voice: { ...state.voice, preserveFormants: value } })),
  },
});
