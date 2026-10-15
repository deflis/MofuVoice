import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AudioPhase,
  DeviceChoices,
  EqBand,
  LevelSettings,
  Settings,
  SettingsView,
} from "../../types";

type SettingsStatus =
  | "loading"
  | "loadFailed"
  | "ready"
  | "saving"
  | "switchingBackend"
  | "closing";

type SettingsStore = {
  status: SettingsStatus;
  settings: Settings | undefined;
  levelDefaults: LevelSettings | undefined;
  devices: string[];
  outputDevices: string[];
  backends: string[];
  deviceError: string;
  resolvedBackend: string;
  audioAvailable: boolean;
  audioBusy: boolean;
  error: string;
  notice: string;
  mount: () => () => void;
  retry: () => void;
  save: () => Promise<void>;
  updateBackend: (backend: string) => Promise<void>;
  updateRouting: (inputChannel: number, outputChannels: number[]) => void;
  update: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  updateLevel: <K extends keyof LevelSettings>(key: K, value: LevelSettings[K]) => void;
  resetLevel: () => void;
  updateEqBand: (index: 0 | 1 | 2, key: keyof EqBand, value: string) => void;
  resetEq: () => void;
  requestClose: () => void;
};

export function requireSettings(settings: Settings | undefined): Settings {
  if (!settings) {
    throw new Error("設定が読み込まれていません。");
  }
  return settings;
}

function applyView(view: SettingsView) {
  return {
    status: "ready" as const,
    settings: view.settings,
    levelDefaults: view.levelDefaults,
    devices: view.devices,
    outputDevices: view.outputDevices,
    backends: view.backends,
    deviceError: view.deviceError,
    resolvedBackend: view.resolvedBackend,
    audioAvailable: view.audioAvailable,
    audioBusy: view.audioBusy,
  };
}

export const useSettingsStore = create<SettingsStore>()((set, get) => {
  let closeRequested = false;
  let applicationClosing = false;
  let delegatedRequestId: number | undefined;
  let loadGeneration = 0;

  const load = () => {
    const generation = ++loadGeneration;
    set({ status: "loading", error: "", notice: "", settings: undefined });
    void invoke<SettingsView>("settings_view")
      .then((view) => {
        if (generation !== loadGeneration) {
          return undefined;
        }
        set({ ...applyView(view), error: "", notice: "" });
        flushClose();
        return undefined;
      })
      .catch((failure: unknown) => {
        if (generation !== loadGeneration) {
          return undefined;
        }
        set({ status: "loadFailed", error: String(failure) });
        closeRequested = false;
        return undefined;
      });
  };

  const hideSettings = () =>
    invoke("close_settings").catch((error: unknown) => set({ error: String(error) }));

  const flushClose = () => {
    if (!closeRequested) {
      return;
    }
    const state = get();
    if (
      state.status === "loading" ||
      state.status === "saving" ||
      state.status === "switchingBackend"
    ) {
      return;
    }
    if (state.status === "loadFailed") {
      closeRequested = false;
      if (!applicationClosing) {
        void hideSettings();
      }
      return;
    }
    if (state.status === "closing" || !state.settings) {
      return;
    }
    closeRequested = false;
    const draft = requireSettings(state.settings);
    set({ status: applicationClosing ? "closing" : "saving", error: "", notice: "" });
    void (async () => {
      await invoke<void>("save_settings", { settings: draft });
      if (applicationClosing) {
        await invoke<void>("close_application");
        applicationClosing = false;
        delegatedRequestId = undefined;
        return;
      }
      set({ status: "ready", notice: "設定を保存しました。" });
      await invoke("close_settings");
    })().catch((error: unknown) => {
      applicationClosing = false;
      set({ status: "ready", error: String(error) });
    });
  };

  return {
    status: "loading",
    settings: undefined,
    levelDefaults: undefined,
    devices: [],
    outputDevices: [],
    backends: [],
    deviceError: "",
    resolvedBackend: "",
    audioAvailable: false,
    audioBusy: false,
    error: "",
    notice: "",
    mount: () => {
      let disposed = false;
      closeRequested = false;
      applicationClosing = false;
      delegatedRequestId = undefined;
      load();

      const unlistenClose = listen("request-close", () => {
        if (!disposed) {
          get().requestClose();
        }
      });
      void unlistenClose.catch((error: unknown) => {
        if (!disposed) {
          set({ error: String(error) });
        }
      });

      const unlistenPhase = listen<AudioPhase>("audio-phase", ({ payload }) => {
        if (disposed) {
          return;
        }
        const busy = payload.phase === "opening" || payload.phase === "running";
        set({ audioBusy: busy });
      });
      void unlistenPhase.catch((error: unknown) => {
        if (!disposed) {
          set({ error: String(error) });
        }
      });

      const unlistenApplicationClose = listen<number>(
        "request-application-close",
        ({ payload: requestId }) => {
          const { settings, status } = get();
          if (disposed || !settings || status === "loadFailed") {
            return;
          }
          void invoke<boolean>("acknowledge_application_close", { requestId })
            .then(async (accepted) => {
              if (!accepted) {
                return undefined;
              }
              if (disposed) {
                await invoke("fallback_application_close", { requestId });
                return undefined;
              }
              delegatedRequestId = requestId;
              applicationClosing = true;
              get().requestClose();
              return undefined;
            })
            .catch((error: unknown) => {
              if (!disposed) {
                set({ error: String(error) });
              }
            });
        },
      );
      void unlistenApplicationClose.catch((error: unknown) => {
        if (!disposed) {
          set({ error: String(error) });
        }
      });

      return () => {
        disposed = true;
        loadGeneration += 1;
        closeRequested = false;
        if (delegatedRequestId !== undefined) {
          void invoke("fallback_application_close", { requestId: delegatedRequestId }).catch(
            () => {},
          );
          delegatedRequestId = undefined;
        }
        void unlistenApplicationClose.then(
          (remove) => remove(),
          () => {},
        );
        void unlistenClose.then(
          (remove) => remove(),
          () => {},
        );
        void unlistenPhase.then(
          (remove) => remove(),
          () => {},
        );
      };
    },
    retry: () => {
      if (get().status === "loadFailed") {
        load();
      }
    },
    save: async () => {
      const state = get();
      if (state.status !== "ready") {
        return;
      }
      set({ status: "saving", error: "", notice: "" });
      try {
        await invoke<void>("save_settings", { settings: requireSettings(get().settings) });
        set({ status: "ready", notice: "設定を保存しました。" });
      } catch (failure) {
        set({ status: "ready", error: String(failure) });
      }
    },
    updateBackend: async (backend) => {
      const state = get();
      if (state.status !== "ready") {
        return;
      }
      set({
        status: "switchingBackend",
        error: "",
        notice: "",
        settings: { ...requireSettings(state.settings), backend, device: "", outputDevice: "" },
        devices: [],
        outputDevices: [],
        audioAvailable: false,
        resolvedBackend: "",
        deviceError: "",
      });
      try {
        const choices = await invoke<DeviceChoices>("device_choices", { backend });
        set({ status: "ready", ...choices });
      } catch (failure) {
        set({ status: "ready", error: String(failure) });
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
    requestClose: () => {
      if (get().status === "closing") {
        return;
      }
      closeRequested = true;
      flushClose();
    },
  };
});
