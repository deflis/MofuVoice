import { invoke } from "@tauri-apps/api/core";
export type Settings = {
  device: string;
  frames: string;
  pitch: string;
  formantShift: string;
  preserveFormants: boolean;
  gate: boolean;
  gateThreshold: string;
  gateAttackMs: string;
  gateReleaseMs: string;
};
export type Snapshot = { busy: boolean; status: string };
export const api = {
  bootstrap: () => invoke<{ settings: Settings; devices: string[] }>("bootstrap"),
  status: () => invoke<Snapshot>("audio_status"),
  start: (settings: Settings, inspect: boolean) =>
    invoke<void>("start_audio", { settings, inspect }),
  stop: () => invoke<void>("stop_audio"),
  save: (settings: Settings) => invoke<void>("save_settings", { settings }),
  spectrum: () => invoke<void>("open_spectrum"),
  close: (settings: Settings) => invoke<void>("close_application", { settings }),
};
