import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAudioStore } from "../../audioStore";

function hideLog() {
  return invoke("close_log").catch((error: unknown) =>
    useAudioStore.setState({ error: String(error) }),
  );
}

export function connectLog() {
  let disposed = false;
  const disconnectAudio = useAudioStore.getState().connect();

  const unlistenClose = listen("request-close", () => {
    if (!disposed) {
      void hideLog();
    }
  });
  void unlistenClose.catch((error: unknown) => useAudioStore.setState({ error: String(error) }));

  return {
    requestClose: () => {
      if (!disposed) {
        void hideLog();
      }
    },
    disconnect: () => {
      disposed = true;
      void unlistenClose.then(
        (remove) => remove(),
        () => {},
      );
      disconnectAudio();
    },
  };
}
