import { Channel, invoke } from "@tauri-apps/api/core";
import type { AudioInfo, AudioLog, AudioPhase, MainView } from "./types";

export type WindowMessage =
  | { type: "main"; view: MainView }
  | { type: "info"; info: AudioInfo }
  | { type: "phase"; phase: AudioPhase }
  | { type: "close" }
  | { type: "applicationClose" }
  | { type: "error"; error: string };

export type Connection = {
  ready: Promise<void>;
  close: () => Promise<void>;
};

function connect<T>(
  command: string,
  disconnect: string,
  channel: Channel<T>,
  receive: (message: T) => void,
  reportError: (error: unknown) => void,
): Connection {
  const connectionId = crypto.randomUUID();
  let closed = false;
  let closing: Promise<void> | undefined;
  // Tauri Channel exposes a single callback setter, not EventTarget methods.
  // oxlint-disable-next-line unicorn/prefer-add-event-listener
  channel.onmessage = (message) => {
    if (!closed) {
      receive(message);
    }
  };
  const ready = invoke<void>(command, { connectionId, channel });
  void ready.catch((error: unknown) => {
    if (!closed) {
      reportError(error);
    }
  });
  async function release() {
    // Registration may still be pending when the connection is closed.
    try {
      await ready;
    } catch {
      return;
    }
    await invoke<void>(disconnect, { connectionId });
  }
  return {
    ready,
    close() {
      closed = true;
      closing ??= release().catch((error: unknown) => {
        closing = undefined;
        throw error;
      });
      return closing;
    },
  };
}

export function connectWindow(
  receive: (message: WindowMessage) => void,
  reportError: (error: unknown) => void,
) {
  return connect(
    "connect_window",
    "disconnect_window",
    new Channel<WindowMessage>(),
    receive,
    reportError,
  );
}
export function connectLogs(
  receive: (log: AudioLog | null) => void,
  reportError: (error: unknown) => void,
) {
  return connect(
    "connect_logs",
    "disconnect_logs",
    new Channel<AudioLog | null>(),
    receive,
    reportError,
  );
}
