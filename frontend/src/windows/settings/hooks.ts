import { useShallow } from "zustand/react/shallow";
import { requireSettings, useSettingsStore } from "./store";

export function useSettingsLocked() {
  return useSettingsStore(
    (state) =>
      state.audioBusy ||
      state.status === "saving" ||
      state.status === "switchingBackend" ||
      state.status === "closing",
  );
}

export function useDeviceSettings() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        device: settings.device,
        outputDevice: settings.outputDevice,
        inputChannel: settings.inputChannel,
        outputChannels: settings.outputChannels,
        backend: settings.backend,
        resolvedBackend: state.resolvedBackend,
        backends: state.backends,
        outputDevices: state.outputDevices,
        updateBackend: state.updateBackend,
        updateRouting: state.updateRouting,
        frames: settings.frames,
        devices: state.devices,
        update: state.update,
      };
    }),
  );
}

export function useVoiceSettings() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        rumbleFilter: settings.rumbleFilter,
        deesser: settings.deesser,
        deesserMode: settings.deesserMode,
        deesserParameters: settings.deesserParameters,
        pitch: settings.pitch,
        formantShift: settings.formantShift,
        preserveFormants: settings.preserveFormants,
        update: state.update,
      };
    }),
  );
}

export function useNoiseGateSettings() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        gate: settings.gate,
        gateThresholdMode: settings.gateThresholdMode,
        gateThresholdDb: settings.gateThresholdDb,
        gateAttackMs: settings.gateAttackMs,
        gateReleaseMs: settings.gateReleaseMs,
        update: state.update,
      };
    }),
  );
}

export function useAutoLevelSettings() {
  return useSettingsStore(
    useShallow((state) => ({
      level: requireSettings(state.settings).level,
      levelDefaults: state.levelDefaults,
      updateLevel: state.updateLevel,
      resetLevel: state.resetLevel,
    })),
  );
}

export function useSaveSettings() {
  return useSettingsStore(
    useShallow((state) => ({
      disabled: state.status !== "ready" || state.audioBusy,
      save: state.save,
    })),
  );
}

export function useEqSettings() {
  return useSettingsStore(
    useShallow((state) => ({
      eq: requireSettings(state.settings).eq,
      update: state.update,
      updateBand: state.updateEqBand,
      reset: state.resetEq,
    })),
  );
}
