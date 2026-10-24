import { createCompressorSlice, type CompressorSlice } from "./compressor/slice";
import { createLimiterSlice, type LimiterSlice } from "./limiter/slice";
import { createDereverbSlice, type DereverbSlice } from "./dereverb/slice";
import { createReverbSlice, type ReverbSlice } from "./reverb/slice";
import { create } from "zustand";
import type { Settings } from "../../types";
import { settingsApi } from "./api";
import * as v from "valibot";
import { settingsSchema } from "./schema";
import { createDeesserSlice, type DeesserSlice } from "./deesser/slice";
import { createDevicesSlice, type DevicesSlice } from "./devices/slice";
import { createEqSlice, type EqSlice } from "./eq/slice";
import { createLevelSlice, type LevelSlice } from "./level/slice";
import { createNoiseSlice, type NoiseSlice } from "./noise/slice";
import { createVoiceSlice, type VoiceSlice } from "./voice/slice";

export type SettingsStore = CompressorSlice &
  LimiterSlice &
  DereverbSlice &
  ReverbSlice &
  DeesserSlice &
  DevicesSlice &
  EqSlice &
  LevelSlice &
  NoiseSlice &
  VoiceSlice & { savedValues: string; markSaved: () => void; save: () => Promise<void> };

export function createSettingsStore(saveSettings: (settings: Settings) => Promise<void>) {
  return create<SettingsStore>()((set, get, store) => ({
    ...createCompressorSlice(set, get, store),
    ...createLimiterSlice(set, get, store),
    ...createDereverbSlice(set, get, store),
    ...createReverbSlice(set, get, store),
    ...createDeesserSlice(set, get, store),
    ...createDevicesSlice(set, get, store),
    ...createEqSlice(set, get, store),
    ...createLevelSlice(set, get, store),
    ...createNoiseSlice(set, get, store),
    ...createVoiceSlice(set, get, store),
    savedValues: "",
    markSaved: () => set({ savedValues: settingsFingerprint(get()) }),
    save: async () => {
      const values = v.parse(settingsSchema, get());
      await saveSettings(values);
      set({ savedValues: settingsFingerprint(values) });
    },
  }));
}
export const useSettingsStore = createSettingsStore(settingsApi.save);

export function settingsFingerprint(values: Settings | SettingsStore): string {
  return JSON.stringify(
    Object.fromEntries(
      (
        [
          "devices",
          "voice",
          "noise",
          "deesser",
          "eq",
          "inputEq",
          "preConversionEq",
          "compressor",
          "limiter",
          "dereverb",
          "reverb",
          "level",
        ] as const
      ).map((key) => [key, values[key]]),
    ),
  );
}
