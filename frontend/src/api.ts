import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AudioInfo, AudioLog, AudioPhase, MainView } from "./types";

type Events = {
  "audio-phase": AudioPhase;
  "main-view": MainView;
  "log-info": AudioInfo;
  "audio-log": AudioLog;
  "request-close": void;
  "request-application-close": number;
};

export function subscribe<K extends keyof Events>(
  event: K,
  receive: (payload: Events[K]) => void,
): Promise<UnlistenFn> {
  return listen<Events[K]>(event, ({ payload }) => receive(payload));
}
