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

export type SettingsStore = DeesserSlice &
  DevicesSlice &
  EqSlice &
  LevelSlice &
  NoiseSlice &
  VoiceSlice & { save: () => Promise<void> };

export function createSettingsStore(saveSettings: (settings: Settings) => Promise<void>) {
  return create<SettingsStore>()((set, get, store) => ({
    ...createDeesserSlice(set, get, store),
    ...createDevicesSlice(set, get, store),
    ...createEqSlice(set, get, store),
    ...createLevelSlice(set, get, store),
    ...createNoiseSlice(set, get, store),
    ...createVoiceSlice(set, get, store),
    save: async () => saveSettings(v.parse(settingsSchema, get())),
  }));
}
export const useSettingsStore = createSettingsStore(settingsApi.save);
