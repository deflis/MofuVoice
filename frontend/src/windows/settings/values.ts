import { copyEqBands } from "./eq/values";
import type { Settings } from "./schema";
import type { SettingsStore } from "./store";
export function loadSettings(state: SettingsStore, values: Settings) {
  return {
    devices: {
      ...state.devices,
      ...values.devices,
      outputChannels: [...values.devices.outputChannels],
    },
    voice: { ...state.voice, ...values.voice },
    noise: { ...state.noise, ...values.noise },
    deesser: { ...state.deesser, ...values.deesser },
    eq: { ...state.eq, ...values.eq, bands: copyEqBands(values.eq.bands) },
    level: { ...state.level, ...values.level },
  };
}
