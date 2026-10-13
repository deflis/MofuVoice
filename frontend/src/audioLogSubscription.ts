import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { AudioLog } from "./types";

// Serialize activation and cleanup so rapid reopen/StrictMode cannot disable a new subscription.
let loggingTransition: Promise<void> = Promise.resolve();

export function subscribeAudioLog(
  onLog: (log: AudioLog) => void,
  onError: (failure: unknown) => void,
): () => void {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;
  let receivedEvent = false;
  const reportError = (failure: unknown) => {
    if (!disposed) {
      onError(failure);
    }
  };
  loggingTransition = loggingTransition
    .then(async () => {
      if (disposed) {
        return undefined;
      }
      const remove = await listen<AudioLog>("audio-log", ({ payload }) => {
        if (!disposed) {
          receivedEvent = true;
          onLog(payload);
        }
      });
      if (disposed) {
        remove();
        return undefined;
      }
      unlisten = remove;
      // Subscribe first so the first pushed update cannot be missed.
      const log = await invoke<AudioLog | null>("set_logging", { logging: true });
      if (!disposed && !receivedEvent && log) {
        onLog(log);
      }
      return undefined;
    })
    .catch(reportError);

  return () => {
    if (disposed) {
      return;
    }
    disposed = true;
    unlisten?.();
    loggingTransition = loggingTransition
      .then(async () => {
        await invoke<AudioLog | null>("set_logging", { logging: false });
        return undefined;
      })
      .catch(onError);
  };
}
