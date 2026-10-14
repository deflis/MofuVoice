import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSettingsStore } from "../../settingsStore";
import { useAudioStore } from "../../audioStore";

function hideSettings() {
  return invoke("close_settings").catch((error: unknown) =>
    useSettingsStore.setState({ error: String(error) }),
  );
}

export function connectSettings() {
  let disposed = false;
  let closeRequested = false;
  let applicationClosing = false;
  let delegatedRequestId: number | undefined;

  const flushClose = () => {
    if (disposed || !closeRequested) {
      return;
    }
    const { settings, saving, switchingBackend, loadFailed } = useSettingsStore.getState();
    const { pending, closing, close } = useAudioStore.getState();
    if (loadFailed) {
      closeRequested = false;
      if (!applicationClosing) {
        void hideSettings();
      }
      return;
    }
    if (!settings || saving || switchingBackend || pending || closing) {
      return;
    }
    closeRequested = false;
    if (applicationClosing) {
      void close(settings).then(() => {
        applicationClosing = false;
        delegatedRequestId = undefined;
        return undefined;
      });
      return;
    }
    void (async () => {
      await useSettingsStore.getState().save();
      if (!useSettingsStore.getState().error) {
        await invoke("close_settings");
      }
    })().catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));
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
  void unlistenClose.catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));

  const unlistenApplicationClose = listen<number>(
    "request-application-close",
    ({ payload: requestId }) => {
      const { settings, loadFailed } = useSettingsStore.getState();
      if (disposed || !settings || loadFailed) {
        return;
      }
      void invoke<boolean>("acknowledge_application_close", { requestId })
        .then(async (accepted) => {
          if (!accepted) {
            return undefined;
          }
          if (disposed) {
            await invoke("fallback_application_close", { requestId });
            return undefined;
          }
          delegatedRequestId = requestId;
          applicationClosing = true;
          requestClose();
          return undefined;
        })
        .catch((error: unknown) => {
          if (!disposed) {
            useSettingsStore.setState({ error: String(error) });
          }
        });
    },
  );
  void unlistenApplicationClose.catch((error: unknown) =>
    useSettingsStore.setState({ error: String(error) }),
  );

  return {
    requestClose,
    disconnect: () => {
      disposed = true;
      closeRequested = false;
      if (delegatedRequestId !== undefined) {
        void invoke("fallback_application_close", { requestId: delegatedRequestId }).catch(
          () => {},
        );
        delegatedRequestId = undefined;
      }
      void unlistenApplicationClose.then(
        (remove) => remove(),
        () => {},
      );
      void unlistenClose.then(
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
