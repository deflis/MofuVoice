import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type LevelSettings = {
  enabled: boolean;
  targetDb: string;
  maxGainDb: string;
  floorDb: string;
  responseMs: string;
  thresholdDb: string;
  ratio: string;
  attackMs: string;
  releaseMs: string;
};
export type Settings = {
  level: LevelSettings;
  device: string;
  outputDevice: string;
  backend: string;
  frames: string;
  pitch: string;
  formantShift: string;
  preserveFormants: boolean;
  gate: boolean;
  gateThresholdMode: "auto" | "manual";
  gateThresholdDb: string;
  gateAttackMs: string;
  gateReleaseMs: string;
};
type DeviceChoices = {
  devices: string[];
  outputDevices: string[];
  audioAvailable: boolean;
  deviceError: string;
  resolvedBackend: string;
};
type Snapshot = { busy: boolean; status: string };

type SettingsStore = {
  settings: Settings | undefined;
  levelDefaults: LevelSettings | undefined;
  devices: string[];
  outputDevices: string[];
  backends: string[];
  deviceError: string;
  resolvedBackend: string;
  spectrumAvailable: boolean;
  audioAvailable: boolean;
  snapshot: Snapshot;
  pending: boolean;
  closing: boolean;
  loadFailed: boolean;
  error: string;
  notice: string;
  connect: () => () => void;
  start: (inspect: boolean) => Promise<void>;
  stop: () => Promise<void>;
  spectrum: () => Promise<void>;
  save: () => Promise<void>;
  close: () => Promise<void>;
  update: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  updateBackend: (backend: string) => Promise<void>;
  updateLevel: <K extends keyof LevelSettings>(key: K, value: LevelSettings[K]) => void;
  resetLevel: () => void;
};

export function requireSettings(settings: Settings | undefined): Settings {
  if (!settings) {
    throw new Error("設定が読み込まれていません。");
  }
  return settings;
}

export const useSettingsStore = create<SettingsStore>()((set, get) => {
  let closeRequested = false;
  const run = async (action: () => Promise<void>) => {
    if (get().pending || get().closing) {
      return;
    }
    set({ pending: true, error: "", notice: "" });
    try {
      await action();
      set({ snapshot: await invoke<Snapshot>("audio_status") });
    } catch (failure) {
      set({ error: String(failure) });
    } finally {
      set({ pending: false });
      if (closeRequested) {
        void get().close();
      }
    }
  };

  return {
    settings: undefined,
    levelDefaults: undefined,
    devices: [],
    outputDevices: [],
    backends: [],
    deviceError: "",
    resolvedBackend: "",
    spectrumAvailable: false,
    audioAvailable: false,
    snapshot: { busy: false, status: "準備中…" },
    pending: false,
    closing: false,
    loadFailed: false,
    error: "",
    notice: "",
    connect: () => {
      set({ loadFailed: false, error: "" });
      let disposed = false;
      let timer: ReturnType<typeof setTimeout> | undefined;
      const reportError = (failure: unknown) => {
        if (!disposed) {
          set({ error: String(failure) });
        }
      };
      const poll = async () => {
        try {
          const snapshot = await invoke<Snapshot>("audio_status");
          if (!disposed) {
            set({ snapshot });
          }
        } catch (failure) {
          reportError(failure);
        }
        if (!disposed) {
          timer = setTimeout(poll, 250);
        }
      };
      const load = async () => {
        try {
          const result = await invoke<
            DeviceChoices & {
              settings: Settings;
              levelDefaults: LevelSettings;
              backends: string[];
              spectrumAvailable: boolean;
            }
          >("bootstrap");
          if (!disposed) {
            set(result);
            if (closeRequested) {
              void get().close();
            }
          }
        } catch (failure) {
          if (!disposed) {
            closeRequested = false;
            set({ loadFailed: true });
          }
          reportError(failure);
        }
      };
      void load();
      void poll();
      const unlisten = listen("request-close", () => {
        if (!disposed) {
          void get().close();
        }
      });
      void unlisten.catch(reportError);
      return () => {
        disposed = true;
        clearTimeout(timer);
        void unlisten.then(
          (remove) => remove(),
          () => {},
        );
      };
    },
    start: (inspect) =>
      run(() =>
        invoke<void>("start_audio", { settings: requireSettings(get().settings), inspect }),
      ),
    stop: () => run(() => invoke<void>("stop_audio")),
    spectrum: () => run(() => invoke<void>("open_spectrum")),
    save: () =>
      run(async () => {
        await invoke<void>("save_settings", { settings: requireSettings(get().settings) });
        set({ notice: "設定を保存しました。" });
      }),
    close: async () => {
      const { settings, pending, closing } = get();
      if (closing) {
        return;
      }
      if (pending || !settings) {
        closeRequested = true;
        return;
      }
      closeRequested = false;
      set({ closing: true, error: "" });
      try {
        await invoke<void>("close_application", { settings });
      } catch (failure) {
        set({ closing: false, error: String(failure) });
      }
    },
    update: (key, value) =>
      set((state) => ({
        settings: { ...requireSettings(state.settings), [key]: value },
        notice: "",
      })),
    updateBackend: (backend) =>
      run(async () => {
        set((state) => ({
          settings: { ...requireSettings(state.settings), backend, device: "", outputDevice: "" },
          devices: [],
          outputDevices: [],
          audioAvailable: false,
          resolvedBackend: "",
          deviceError: "",
          notice: "",
        }));
        set(await invoke<DeviceChoices>("device_choices", { backend }));
      }),
    updateLevel: (key, value) =>
      set((state) => {
        const settings = requireSettings(state.settings);
        return {
          settings: { ...settings, level: { ...settings.level, [key]: value } },
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
