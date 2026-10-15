import { invoke } from "@tauri-apps/api/core";
import { subscribe } from "../../api";
import type { AudioInfo, AudioLog } from "../../types";

export const logApi = {
  getView: () => invoke<AudioInfo>("log_view"),
  setLogging: (logging: boolean) => invoke<AudioLog | null>("set_logging", { logging }),
  hide: () => invoke<void>("close_log"),
  onInfo: (receive: (info: AudioInfo) => void) => subscribe("log-info", receive),
  onLog: (receive: (log: AudioLog) => void) => subscribe("audio-log", receive),
  onClose: (receive: () => void) => subscribe("request-close", receive),
};

export type LogApi = typeof logApi;
