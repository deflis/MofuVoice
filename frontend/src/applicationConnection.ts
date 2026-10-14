import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Bootstrap, Settings } from "./types";
import { useSettingsStore } from "./settingsStore";
import { useAudioStore } from "./audioStore";

// Coordinate application shutdown across independently owned settings/audio operations.
export function connectApplication(settingsWindow = false) {
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
      return;
    }
    if (!settings || saving || switchingBackend || pending || closing) {
      return;
    }
    closeRequested = false;
    if (settingsWindow && !applicationClosing) {
      void (async () => {
        await useSettingsStore.getState().save();
        if (!useSettingsStore.getState().error) {
          await invoke("close_settings");
        }
      })().catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));
    } else {
      void close(settings).then(() => {
        applicationClosing = false;
        delegatedRequestId = undefined;
        return undefined;
      });
    }
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
  const saved = settingsWindow
    ? undefined
    : listen<Settings>("settings-saved", ({ payload }) => {
        if (!disposed) {
          useSettingsStore.setState({ settings: payload });
          // Refresh device choices when the settings window changed the audio backend.
          void invoke<Bootstrap>("bootstrap")
            .then((result) => {
              if (!disposed && useSettingsStore.getState().settings === payload) {
                useSettingsStore.setState(result);
              }
              return undefined;
            })
            .catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));
        }
      });
  void saved?.catch((error: unknown) => useSettingsStore.setState({ error: String(error) }));
  const applicationClose = settingsWindow
    ? listen<number>("request-application-close", ({ payload: requestId }) => {
        const { settings, loadFailed } = useSettingsStore.getState();
        if (disposed || !settings || loadFailed) {
          // The native deadline returns shutdown to main when this window cannot save a draft.
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
      })
    : undefined;
  void applicationClose?.catch((error: unknown) =>
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
      void applicationClose?.then(
        (remove) => remove(),
        () => {},
      );
      void saved?.then(
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
