import { invoke } from "@tauri-apps/api/core";
import { subscribe } from "../../api";
import type { AudioPhase, DeviceChoices, Settings, SettingsView } from "../../types";

export const settingsApi = {
  getView: () => invoke<SettingsView>("settings_view"),
  save: (settings: Settings) => invoke<void>("save_settings", { settings }),
  deviceChoices: (backend: string) => invoke<DeviceChoices>("device_choices", { backend }),
  hide: () => invoke<void>("close_settings"),
  closeApplication: () => invoke<void>("close_application"),
  acknowledgeClose: (requestId: number) =>
    invoke<boolean>("acknowledge_application_close", { requestId }),
  fallbackClose: (requestId: number) => invoke<void>("fallback_application_close", { requestId }),
  onPhase: (receive: (phase: AudioPhase) => void) => subscribe("audio-phase", receive),
  onClose: (receive: () => void) => subscribe("request-close", receive),
  onApplicationClose: (receive: (requestId: number) => void) =>
    subscribe("request-application-close", receive),
};

export type SettingsApi = typeof settingsApi;
