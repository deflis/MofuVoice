import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { VoiceValues } from "./schema";

type VoiceState = VoiceValues & {
  setEnabled: (value: boolean) => void;
  setPitchUnit: (value: VoiceValues["pitchUnit"]) => void;
  setMathPrecision: (value: VoiceValues["mathPrecision"]) => void;
  setPsolaPitchEstimator: (value: VoiceValues["psolaPitchEstimator"]) => void;
  setPsolaAnalysisWindow: (value: VoiceValues["psolaAnalysisWindow"]) => void;
  setPsolaSynthesisWindow: (value: VoiceValues["psolaSynthesisWindow"]) => void;
  setAnalysisWindow: (value: VoiceValues["analysisWindow"]) => void;
  setSynthesisWindow: (value: VoiceValues["synthesisWindow"]) => void;
  setPitchAlgorithm: (value: VoiceValues["pitchAlgorithm"]) => void;
  setPitch: (value: VoiceValues["pitch"]) => void;
  setFormantShift: (value: VoiceValues["formantShift"]) => void;
  setPreserveFormants: (value: VoiceValues["preserveFormants"]) => void;
};
export type VoiceSlice = { voice: VoiceState };
export const createVoiceSlice: StateCreator<SettingsStore, [], [], VoiceSlice> = (set) => ({
  voice: {
    enabled: true,
    setEnabled: (value) => set((state) => ({ voice: { ...state.voice, enabled: value } })),
    pitchUnit: "semitones",
    setPitchUnit: (value) => set((state) => ({ voice: { ...state.voice, pitchUnit: value } })),
    pitchAlgorithm: "phase-locked",
    setPitchAlgorithm: (value) =>
      set((state) => ({
        voice: {
          ...state.voice,
          pitchAlgorithm: value,
          ...(value === "td-psola" || value === "td-psola-resampled"
            ? { preserveFormants: true }
            : {}),
        },
      })),
    mathPrecision: "standard",
    setMathPrecision: (value) =>
      set((state) => ({ voice: { ...state.voice, mathPrecision: value } })),
    psolaPitchEstimator: "yin",
    setPsolaPitchEstimator: (value) =>
      set((state) => ({ voice: { ...state.voice, psolaPitchEstimator: value } })),
    psolaAnalysisWindow: 2048,
    psolaSynthesisWindow: 512,
    setPsolaAnalysisWindow: (value) =>
      set((state) => ({
        voice: {
          ...state.voice,
          psolaAnalysisWindow: value,
          psolaSynthesisWindow:
            value === 1024 && state.voice.psolaSynthesisWindow === 2048
              ? 1024
              : state.voice.psolaSynthesisWindow,
        },
      })),
    setPsolaSynthesisWindow: (value) =>
      set((state) => ({ voice: { ...state.voice, psolaSynthesisWindow: value } })),
    analysisWindow: 2048,
    synthesisWindow: 512,
    setAnalysisWindow: (value) =>
      set((state) => ({
        voice: {
          ...state.voice,
          analysisWindow: value,
          synthesisWindow:
            value === 1024 && state.voice.synthesisWindow === 2048
              ? 1024
              : state.voice.synthesisWindow,
        },
      })),
    setSynthesisWindow: (value) =>
      set((state) => ({ voice: { ...state.voice, synthesisWindow: value } })),
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
