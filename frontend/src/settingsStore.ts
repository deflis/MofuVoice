import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Bootstrap, DeviceChoices, EqBand, LevelSettings, Settings } from "./types";
export type { LevelSettings, Settings } from "./types";

type SettingsStore = {
  settings: Settings | undefined;
  levelDefaults: LevelSettings | undefined;
  devices: string[];
  saving: boolean;
  switchingBackend: boolean;
  loadFailed: boolean;
  outputDevices: string[];
  backends: string[];
  deviceError: string;
  resolvedBackend: string;
  spectrumAvailable: boolean;
  audioAvailable: boolean;
  error: string;
  notice: string;
  connect: () => () => void;
  save: () => Promise<void>;
  updateBackend: (backend: string) => Promise<void>;
  updateRouting: (inputChannel: number, outputChannels: number[]) => void;
  update: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  updateLevel: <K extends keyof LevelSettings>(key: K, value: LevelSettings[K]) => void;
  resetLevel: () => void;
  updateEqBand: (index: 0 | 1 | 2, key: keyof EqBand, value: string) => void;
  resetEq: () => void;
};

export function requireSettings(settings: Settings | undefined): Settings {
  if (!settings) {
    throw new Error("設定が読み込まれていません。");
  }
  return settings;
}

export const useSettingsStore = create<SettingsStore>()((set, get) => {
  return {
    settings: undefined,
    levelDefaults: undefined,
    devices: [],
    saving: false,
    switchingBackend: false,
    loadFailed: false,
    outputDevices: [],
    backends: [],
    deviceError: "",
    resolvedBackend: "",
    spectrumAvailable: false,
    audioAvailable: false,
    error: "",
    notice: "",
    connect: () => {
      set({ loadFailed: false, error: "" });
      let disposed = false;
      const reportError = (failure: unknown) => {
        if (!disposed) {
          set({ error: String(failure) });
        }
      };
      const load = async () => {
        try {
          const result = await invoke<Bootstrap>("bootstrap");
          if (!disposed) {
            set(result);
          }
        } catch (failure) {
          if (!disposed) {
            set({ loadFailed: true });
          }
          reportError(failure);
        }
      };
      void load();
      return () => {
        disposed = true;
      };
    },
    save: async () => {
      if (get().saving || get().switchingBackend) {
        return;
      }
      set({ saving: true, error: "", notice: "" });
      try {
        await invoke<void>("save_settings", { settings: requireSettings(get().settings) });
        set({ notice: "設定を保存しました。" });
      } catch (failure) {
        set({ error: String(failure) });
      } finally {
        set({ saving: false });
      }
    },
    updateBackend: async (backend) => {
      if (get().saving || get().switchingBackend) {
        return;
      }
      set((state) => ({
        switchingBackend: true,
        error: "",
        notice: "",
        settings: { ...requireSettings(state.settings), backend, device: "", outputDevice: "" },
        devices: [],
        outputDevices: [],
        audioAvailable: false,
        resolvedBackend: "",
        deviceError: "",
      }));
      try {
        set(await invoke<DeviceChoices>("device_choices", { backend }));
      } catch (failure) {
        set({ error: String(failure) });
      } finally {
        set({ switchingBackend: false });
      }
    },
    updateRouting: (inputChannel, outputChannels) =>
      set((state) => ({
        settings: {
          ...requireSettings(state.settings),
          inputChannel,
          outputChannels: [...outputChannels],
        },
        notice: "",
      })),
    update: (key, value) =>
      set((state) => ({
        settings: { ...requireSettings(state.settings), [key]: value },
        notice: "",
      })),
    updateLevel: (key, value) =>
      set((state) => {
        const settings = requireSettings(state.settings);
        return {
          settings: { ...settings, level: { ...settings.level, [key]: value } },
          notice: "",
        };
      }),
    updateEqBand: (index, key, value) =>
      set((state) => {
        const settings = requireSettings(state.settings);
        const bands: Settings["eq"]["bands"] = [...settings.eq.bands];
        bands[index] = { ...bands[index], [key]: value };
        return { settings: { ...settings, eq: { ...settings.eq, bands } }, notice: "" };
      }),
    resetEq: () =>
      set((state) => {
        const settings = requireSettings(state.settings);
        return {
          settings: {
            ...settings,
            eq: {
              ...settings.eq,
              bands: [
                { frequency: "250", gainDb: "0", q: "0.7" },
                { frequency: "3000", gainDb: "0", q: "0.8" },
                { frequency: "8000", gainDb: "0", q: "0.7" },
              ],
              outputDb: "0",
            },
          },
          notice: "",
        };
      }),
    resetLevel: () =>
      set((state) => {
        const settings = requireSettings(state.settings);
        if (!state.levelDefaults) {
          throw new Error("既定値が読み込まれていません。");
        }
        return {
          notice: "",
          settings: {
            ...settings,
            level: { ...state.levelDefaults, enabled: settings.level.enabled },
          },
        };
      }),
  };
});
