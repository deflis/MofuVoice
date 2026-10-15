import { useShallow } from "zustand/react/shallow";
import { canEditSettings, requireSettings, useSettingsStore } from "./store";

export function useSettingsLocked() {
  return useSettingsStore((state) => !canEditSettings(state));
}

export function useDeviceSelection() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        device: settings.device,
        outputDevice: settings.outputDevice,
        resolvedBackend: state.resolvedBackend,
        devices: state.devices,
        outputDevices: state.outputDevices,
        update: state.update,
      };
    }),
  );
}

export function useSampleRateSettings() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        device: settings.device,
        outputDevice: settings.outputDevice,
        resolvedBackend: state.resolvedBackend,
        sampleRate: settings.sampleRate,
        update: state.update,
      };
    }),
  );
}

export function useChannelRouting() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        inputChannel: settings.inputChannel,
        outputChannels: settings.outputChannels,
        update: state.update,
        updateRouting: state.updateRouting,
      };
    }),
  );
}

export function useBackendSettings() {
  return useSettingsStore(
    useShallow((state) => ({
      backend: requireSettings(state.settings).backend,
      backends: state.backends,
      updateBackend: state.updateBackend,
    })),
  );
}

export function useBufferSettings() {
  return useSettingsStore(
    useShallow((state) => ({
      frames: requireSettings(state.settings).frames,
      update: state.update,
    })),
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
      disabled: !canEditSettings(state),
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
