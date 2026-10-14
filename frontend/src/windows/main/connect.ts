import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Bootstrap, Settings } from "../../types";
import { useSettingsStore } from "../../settingsStore";
import { useAudioStore } from "../../audioStore";

export function connectMain() {
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
  const disconnectAudio = useAudioStore.getState().connect();

  const unlistenClose = listen("request-close", () => {
    if (!disposed) {
      requestClose();
    }
  });
  void unlistenClose.catch((error: unknown) => useAudioStore.setState({ error: String(error) }));

  const unlistenSaved = listen<Settings>("settings-saved", ({ payload }) => {
    if (disposed) {
      return;
    }
    useSettingsStore.setState({ settings: payload });
    void invoke<Bootstrap>("bootstrap")
      .then((result) => {
        if (!disposed && useSettingsStore.getState().settings === payload) {
          useSettingsStore.setState(result);
        }
        return undefined;
      })
      .catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));
  });
  void unlistenSaved.catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));

  return {
    requestClose,
    disconnect: () => {
      disposed = true;
      closeRequested = false;
      void unlistenClose.then(
        (remove) => remove(),
        () => {},
      );
      void unlistenSaved.then(
        (remove) => remove(),
        () => {},
      );
      removeSettings();
      removeAudio();
      disconnectAudio();
      disconnectSettings();
    },
  };
}
