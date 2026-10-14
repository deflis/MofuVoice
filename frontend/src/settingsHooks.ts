import { useShallow } from "zustand/react/shallow";
import { useAudioStore } from "./audioStore";
import { requireSettings, useSettingsStore } from "./settingsStore";

export function useSettingsReady() {
  return useSettingsStore((state) => state.settings !== undefined);
}

export function useSettingsLoadFailed() {
  return useSettingsStore((state) => state.loadFailed);
}

export function useSettingsLocked() {
  const audioLocked = useAudioStore(
    (state) => state.pending || state.closing || state.snapshot.busy,
  );
  const saving = useSettingsStore((state) => state.saving || state.switchingBackend);
  return audioLocked || saving;
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
  const operating = useAudioStore((state) => state.pending || state.closing);
  const { saving, save } = useSettingsStore(
    useShallow((state) => ({ saving: state.saving || state.switchingBackend, save: state.save })),
  );
  return { disabled: operating || saving, save };
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
