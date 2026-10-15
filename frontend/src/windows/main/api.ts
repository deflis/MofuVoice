import { invoke } from "@tauri-apps/api/core";
import { subscribe } from "../../api";
import type { AudioPhase, MainView } from "../../types";
import { selectSettingsSection, type SettingsSection } from "../settings/navigation";

export const mainApi = {
  getView: () => invoke<MainView>("main_view"),
  start: (inspect: boolean) => invoke<void>("start_audio", { inspect }),
  stop: () => invoke<void>("stop_audio"),
  spectrum: () => invoke<void>("open_spectrum"),
  openSettings: async (section: SettingsSection) => {
    selectSettingsSection(section);
    await invoke<void>("open_settings");
  },
  openLog: () => invoke<void>("open_log"),
  close: () => invoke<void>("close_application"),
  onPhase: (receive: (phase: AudioPhase) => void) => subscribe("audio-phase", receive),
  onView: (receive: (view: MainView) => void) => subscribe("main-view", receive),
  onClose: (receive: () => void) => subscribe("request-close", receive),
};

export type MainApi = typeof mainApi;
