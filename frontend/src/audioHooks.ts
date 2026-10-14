import { useShallow } from "zustand/react/shallow";
import { useAudioStore } from "./audioStore";
import { requireSettings, useSettingsStore } from "./settingsStore";

export function useAudioControls() {
  const { settings, saving, audioAvailable, spectrumAvailable, resolvedBackend } = useSettingsStore(
    useShallow((state) => ({
      settings: requireSettings(state.settings),
      saving: state.saving || state.switchingBackend,
      audioAvailable: state.audioAvailable,
      spectrumAvailable: state.spectrumAvailable,
      resolvedBackend: state.resolvedBackend,
    })),
  );
  const audio = useAudioStore(
    useShallow((state) => ({
      pending: state.pending,
      closing: state.closing,
      busy: state.snapshot.busy,
      start: state.start,
      stop: state.stop,
      spectrum: state.spectrum,
    })),
  );
  const operating = audio.pending || audio.closing || saving;
  return {
    startDisabled:
      operating ||
      audio.busy ||
      !audioAvailable ||
      (resolvedBackend === "asio" && !settings.device),
    stopDisabled: operating || !audio.busy,
    spectrumDisabled: operating || !spectrumAvailable,
    start: (inspect: boolean) => audio.start(settings, inspect),
    stop: audio.stop,
    spectrum: audio.spectrum,
  };
}

export function useHeaderStatus() {
  return useAudioStore((state) =>
    state.closing
      ? "終了中"
      : {
          ready: "停止中",
          opening: "デバイスを起動中",
          running: "音声処理中",
          stopped: "停止中",
          inspected: "停止中",
          error: "エラー",
        }[state.snapshot.info.phase ?? "ready"],
  );
}

export function useNotifications() {
  const error = useAudioStore((state) => state.error || state.snapshot.info.error || "");
  const settings = useSettingsStore(
    useShallow((state) => ({ error: state.error || state.deviceError, notice: state.notice })),
  );
  return { error: [error, settings.error].filter(Boolean).join("\n"), notice: settings.notice };
}

export function useStatusDetails() {
  return useAudioStore((state) => state.snapshot.info);
}
