import { useSettingsStore } from "./settingsStore";
import { useAudioStore } from "./audioStore";

// Coordinate application shutdown across independently owned settings/audio operations.
export function connectApplication() {
  let disposed = false;
  let closeRequested = false;
  const flushClose = () => {
    if (disposed || !closeRequested) {
      return;
    }
    const { settings, saving, switchingBackend, loadFailed } = useSettingsStore.getState();
    const { pending, closing, close } = useAudioStore.getState();
    if (loadFailed) {
      closeRequested = false;
      return;
    }
    if (!settings || saving || switchingBackend || pending || closing) {
      return;
    }
    closeRequested = false;
    void close(settings);
  };
  const requestClose = () => {
    if (disposed || useAudioStore.getState().closing) {
      return;
    }
    closeRequested = true;
    flushClose();
  };
  const removeSettings = useSettingsStore.subscribe(flushClose);
  const removeAudio = useAudioStore.subscribe(flushClose);
  const disconnectSettings = useSettingsStore.getState().connect();
  const disconnectAudio = useAudioStore.getState().connect(requestClose);
  return {
    requestClose,
    disconnect: () => {
      disposed = true;
      closeRequested = false;
      removeSettings();
      removeAudio();
      disconnectAudio();
      disconnectSettings();
    },
  };
}
