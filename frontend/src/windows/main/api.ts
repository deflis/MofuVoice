import { invoke } from "@tauri-apps/api/core";
import { selectSettingsSection, type SettingsSection } from "../settings/navigation";

export const mainApi = {
  start: (inspect: boolean) => invoke<void>("start_audio", { inspect }),
  stop: () => invoke<void>("stop_audio"),
  spectrum: () => invoke<void>("open_spectrum"),
  openSettings: async (section: SettingsSection) => {
    selectSettingsSection(section);
    await invoke<void>("open_settings");
  },
  openLog: () => invoke<void>("open_log"),
  close: () => invoke<void>("close_application"),
};

export type MainApi = typeof mainApi;
