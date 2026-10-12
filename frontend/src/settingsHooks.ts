import { useShallow } from "zustand/react/shallow";
import { requireSettings, useSettingsStore } from "./settingsStore";

export function useSettingsReady() {
  return useSettingsStore((state) => state.settings !== undefined);
}

export function useSettingsLocked() {
  return useSettingsStore((state) => state.pending || state.closing || state.snapshot.busy);
}

export function useDeviceSettings() {
  return useSettingsStore(
    useShallow((state) => {
      const settings = requireSettings(state.settings);
      return {
        device: settings.device,
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
        gateThreshold: settings.gateThreshold,
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

export function useAudioControls() {
  return useSettingsStore(
    useShallow((state) => {
      const operating = state.pending || state.closing;
      const busy = state.snapshot.busy;
      return {
        startDisabled: operating || busy || !requireSettings(state.settings).device,
        stopDisabled: operating || !busy,
        spectrumDisabled: operating,
        firstLine: state.snapshot.status.split("\n")[0],
        start: state.start,
        stop: state.stop,
        spectrum: state.spectrum,
      };
    }),
  );
}

export function useSaveSettings() {
  return useSettingsStore(
    useShallow((state) => ({
      disabled: state.pending || state.closing,
      save: state.save,
    })),
  );
}

export function useHeaderStatus() {
  return useSettingsStore((state) => {
    if (state.closing) {
      return "終了中";
    }
    if (state.snapshot.busy) {
      return "処理中";
    }
    return "停止中";
  });
}

export function useNotifications() {
  return useSettingsStore(useShallow((state) => ({ error: state.error, notice: state.notice })));
}

export function useStatusDetails() {
  return useSettingsStore((state) => state.snapshot.status);
}
