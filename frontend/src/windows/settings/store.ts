import { settingsApi, type SettingsApi } from "./api";
import { create } from "zustand";
import type { EqBand, LevelSettings, Settings, SettingsView } from "../../types";

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

export function canEditSettings(state: Pick<SettingsStore, "status" | "audioBusy">): boolean {
  return state.status === "ready" && !state.audioBusy;
}

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

export function createSettingsStore(api: SettingsApi = settingsApi) {
  return create<SettingsStore>()((set, get) => {
    let lifetime = 0;
    let disconnectCurrent: (() => void) | undefined;
    let closeRequested = false;
    let applicationClosing = false;
    let delegatedRequestId: number | undefined;
    let loadGeneration = 0;
    let phaseSeen = false;

    const load = () => {
      const generation = ++loadGeneration;
      set({ status: "loading", error: "", notice: "", settings: undefined });
      void api
        .getView()
        .then((view) => {
          if (generation !== loadGeneration) {
            return undefined;
          }
          set((state) => ({
            ...applyView(view),
            audioBusy: phaseSeen ? state.audioBusy : view.audioBusy,
            error: "",
            notice: "",
          }));
          flushClose();
          return undefined;
        })
        .catch((failure: unknown) => {
          if (generation !== loadGeneration) {
            return undefined;
          }
          set({ status: "loadFailed", error: String(failure) });
          flushClose();
          return undefined;
        });
    };

    const hideSettings = () => {
      const generation = lifetime;
      return api.hide().catch((error: unknown) => {
        if (generation === lifetime) {
          set({ error: String(error) });
        }
      });
    };

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
      const generation = lifetime;
      set({ status: applicationClosing ? "closing" : "saving", error: "", notice: "" });
      void (async () => {
        await api.save(draft);
        if (generation !== lifetime) {
          return;
        }
        if (applicationClosing) {
          await api.closeApplication();
          if (generation !== lifetime) {
            return;
          }
          applicationClosing = false;
          delegatedRequestId = undefined;
          closeRequested = false;
          return;
        }
        // Drop close requests queued while this automatic save was in flight so a
        // remounted settings window is not immediately saved/hidden again.
        closeRequested = false;
        set({ status: "ready", notice: "設定を保存しました。" });
        await api.hide();
      })().catch((error: unknown) => {
        if (generation !== lifetime) {
          return;
        }
        applicationClosing = false;
        delegatedRequestId = undefined;
        closeRequested = false;
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
        disconnectCurrent?.();
        const mounted = ++lifetime;
        let disposed = false;
        let acknowledging = false;
        closeRequested = false;
        applicationClosing = false;
        delegatedRequestId = undefined;
        phaseSeen = false;

        const unlistenClose = api.onClose(() => {
          if (!disposed) {
            get().requestClose();
          }
        });
        void unlistenClose.catch((error: unknown) => {
          if (!disposed) {
            set({ error: String(error) });
          }
        });

        const unlistenPhase = api.onPhase((payload) => {
          if (disposed) {
            return;
          }
          phaseSeen = true;
          set({ audioBusy: payload.busy });
        });
        void unlistenPhase.catch((error: unknown) => {
          if (!disposed) {
            set({ error: String(error) });
          }
        });

        const unlistenApplicationClose = api.onApplicationClose((requestId) => {
          const { settings, status } = get();
          if (
            disposed ||
            acknowledging ||
            applicationClosing ||
            !settings ||
            status === "loadFailed" ||
            status === "closing"
          ) {
            return;
          }
          acknowledging = true;
          void api
            .acknowledgeClose(requestId)
            .then(async (accepted) => {
              if (!disposed) {
                acknowledging = false;
              }
              if (!accepted) {
                return undefined;
              }
              if (disposed) {
                await api.fallbackClose(requestId);
                return undefined;
              }
              delegatedRequestId = requestId;
              applicationClosing = true;
              get().requestClose();
              return undefined;
            })
            .catch((error: unknown) => {
              if (!disposed) {
                acknowledging = false;
                set({ error: String(error) });
              }
            });
        });
        void unlistenApplicationClose.catch((error: unknown) => {
          if (!disposed) {
            set({ error: String(error) });
          }
        });

        load();

        disconnectCurrent = () => {
          if (disposed) {
            return;
          }
          disposed = true;
          if (lifetime === mounted) {
            lifetime += 1;
          }
          loadGeneration += 1;
          closeRequested = false;
          phaseSeen = false;
          if (delegatedRequestId !== undefined) {
            void api.fallbackClose(delegatedRequestId).catch(() => {});
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
        return disconnectCurrent;
      },
      retry: () => {
        if (get().status === "loadFailed") {
          load();
        }
      },
      save: async () => {
        const generation = lifetime;
        const state = get();
        if (!canEditSettings(state)) {
          return;
        }
        set({ status: "saving", error: "", notice: "" });
        try {
          await api.save(requireSettings(state.settings));
          if (generation !== lifetime) {
            return;
          }
          set({ status: "ready", notice: "設定を保存しました。" });
        } catch (failure) {
          if (generation !== lifetime) {
            return;
          }
          set({ status: "ready", error: String(failure) });
        }
        flushClose();
      },
      updateBackend: async (backend) => {
        const generation = lifetime;
        const state = get();
        if (!canEditSettings(state)) {
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
          const choices = await api.deviceChoices(backend);
          if (generation !== lifetime) {
            return;
          }
          set({ status: "ready", ...choices });
        } catch (failure) {
          if (generation !== lifetime) {
            return;
          }
          set({ status: "ready", error: String(failure) });
        }
        flushClose();
      },
      updateRouting: (inputChannel, outputChannels) =>
        set((state) =>
          !canEditSettings(state)
            ? state
            : {
                settings: {
                  ...requireSettings(state.settings),
                  inputChannel,
                  outputChannels: [...outputChannels],
                },
                notice: "",
              },
        ),
      update: (key, value) => {
        if (key === "backend" && typeof value === "string") {
          void get().updateBackend(value);
          return;
        }
        set((state) =>
          !canEditSettings(state)
            ? state
            : {
                settings: { ...requireSettings(state.settings), [key]: value },
                notice: "",
              },
        );
      },
      updateLevel: (key, value) =>
        set((state) => {
          if (!canEditSettings(state)) {
            return state;
          }
          const settings = requireSettings(state.settings);
          return {
            settings: { ...settings, level: { ...settings.level, [key]: value } },
            notice: "",
          };
        }),
      updateEqBand: (index, key, value) =>
        set((state) => {
          if (!canEditSettings(state)) {
            return state;
          }
          const settings = requireSettings(state.settings);
          const bands: Settings["eq"]["bands"] = [...settings.eq.bands];
          bands[index] = { ...bands[index], [key]: value };
          return { settings: { ...settings, eq: { ...settings.eq, bands } }, notice: "" };
        }),
      resetEq: () =>
        set((state) => {
          if (!canEditSettings(state)) {
            return state;
          }
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
          if (!canEditSettings(state)) {
            return state;
          }
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
}

export const useSettingsStore = createSettingsStore();
