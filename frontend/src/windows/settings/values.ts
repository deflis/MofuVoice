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
    inputEq: { ...state.inputEq, ...values.inputEq, bands: copyEqBands(values.inputEq.bands) },
    preConversionEq: {
      ...state.preConversionEq,
      ...values.preConversionEq,
      bands: copyEqBands(values.preConversionEq.bands),
    },
    compressor: { ...state.compressor, ...values.compressor },
    limiter: { ...state.limiter, ...values.limiter },
    dereverb: { ...state.dereverb, ...values.dereverb },
    reverb: { ...state.reverb, ...values.reverb },
  };
}
