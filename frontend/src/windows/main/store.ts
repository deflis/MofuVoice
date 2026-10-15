import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AudioPhase, MainView } from "../../types";
import { openSettings, type SettingsSection } from "../settings/navigation";

export type MainMachine =
  | { status: "bootstrapping" }
  | { status: "bootstrapFailed"; error: string }
  | { status: "idle"; view: MainView }
  | { status: "starting"; view: MainView; inspect: boolean }
  | { status: "running"; view: MainView }
  | { status: "stopping"; view: MainView }
  | { status: "faulted"; view: MainView; error: string }
  | { status: "closing" };

type MainStore = {
  machine: MainMachine;
  notice: string;
  mount: () => () => void;
  retry: () => void;
  start: (inspect: boolean) => Promise<void>;
  stop: () => Promise<void>;
  spectrum: () => Promise<void>;
  openSettings: (section: SettingsSection) => Promise<void>;
  openLog: () => Promise<void>;
  requestClose: () => void;
};

function withView(machine: MainMachine, view: MainView): MainMachine {
  switch (machine.status) {
    case "idle":
    case "starting":
    case "running":
    case "stopping":
    case "faulted":
      return { ...machine, view };
    default:
      return machine;
  }
}

function applyPhase(machine: MainMachine, phase: AudioPhase): MainMachine {
  if (
    machine.status === "closing" ||
    machine.status === "bootstrapping" ||
    machine.status === "bootstrapFailed"
  ) {
    return machine;
  }
  if (phase.phase === "error") {
    return {
      status: "faulted",
      view: machine.view,
      error: phase.error ?? "音声処理でエラーが発生しました。",
    };
  }
  if (machine.status === "starting") {
    if (phase.phase === "running") {
      return { status: "running", view: machine.view };
    }
    if (phase.phase === "opening") {
      return machine;
    }
    if (phase.phase === "ready" || phase.phase === "stopped" || phase.phase === "inspected") {
      return { status: "idle", view: machine.view };
    }
  }
  if (machine.status === "stopping") {
    if (phase.phase === "ready" || phase.phase === "stopped" || phase.phase === "inspected") {
      return { status: "idle", view: machine.view };
    }
    return machine;
  }
  if (phase.phase === "running" || phase.phase === "opening") {
    return { status: "running", view: machine.view };
  }
  if (phase.phase === "ready" || phase.phase === "stopped" || phase.phase === "inspected") {
    return { status: "idle", view: machine.view };
  }
  return {
    status: "faulted",
    view: machine.view,
    error: `不正な状態遷移です（${machine.status} ← ${phase.phase}）。`,
  };
}

export const useMainStore = create<MainStore>()((set, get) => {
  let closeRequested = false;
  let loadGeneration = 0;

  const loadView = () => {
    const generation = ++loadGeneration;
    set({ machine: { status: "bootstrapping" }, notice: "" });
    void invoke<MainView>("main_view")
      .then((view) => {
        if (generation !== loadGeneration) {
          return undefined;
        }
        set({ machine: { status: "idle", view } });
        flushClose();
        return undefined;
      })
      .catch((failure: unknown) => {
        if (generation !== loadGeneration) {
          return undefined;
        }
        set({ machine: { status: "bootstrapFailed", error: String(failure) } });
        closeRequested = false;
        return undefined;
      });
  };

  const beginClose = () => {
    closeRequested = false;
    set({ machine: { status: "closing" }, notice: "" });
    void invoke<void>("close_application").catch((failure: unknown) => {
      set({
        machine: {
          status: "faulted",
          view: {
            canStart: false,
            spectrumAvailable: false,
            setup: {
              audioAvailable: false,
              resolvedBackend: "",
              inputConfigured: false,
              outputConfigured: false,
              rememberedDeviceMissing: false,
            },
          },
          error: String(failure),
        },
      });
    });
  };

  const flushClose = () => {
    if (!closeRequested) {
      return;
    }
    const { machine } = get();
    if (
      machine.status === "bootstrapping" ||
      machine.status === "starting" ||
      machine.status === "stopping"
    ) {
      return;
    }
    if (machine.status === "bootstrapFailed") {
      closeRequested = false;
      return;
    }
    if (machine.status === "closing") {
      return;
    }
    beginClose();
  };

  return {
    machine: { status: "bootstrapping" },
    notice: "",
    mount: () => {
      let disposed = false;
      closeRequested = false;
      loadView();

      const reportListenError = (failure: unknown) => {
        if (disposed) {
          return;
        }
        const { machine } = get();
        if (
          machine.status === "idle" ||
          machine.status === "running" ||
          machine.status === "faulted"
        ) {
          set({
            machine: {
              status: "faulted",
              view: machine.view,
              error: String(failure),
            },
          });
        }
      };

      const unlistenPhase = listen<AudioPhase>("audio-phase", ({ payload }) => {
        if (disposed) {
          return;
        }
        set((state) => ({ machine: applyPhase(state.machine, payload) }));
        flushClose();
      });
      void unlistenPhase.catch(reportListenError);

      const unlistenView = listen<MainView>("main-view", ({ payload }) => {
        if (disposed) {
          return;
        }
        set((state) => ({ machine: withView(state.machine, payload) }));
      });
      void unlistenView.catch(reportListenError);

      const unlistenClose = listen("request-close", () => {
        if (!disposed) {
          get().requestClose();
        }
      });
      void unlistenClose.catch(reportListenError);

      return () => {
        disposed = true;
        loadGeneration += 1;
        closeRequested = false;
        void unlistenPhase.then(
          (remove) => remove(),
          () => {},
        );
        void unlistenView.then(
          (remove) => remove(),
          () => {},
        );
        void unlistenClose.then(
          (remove) => remove(),
          () => {},
        );
      };
    },
    retry: () => {
      if (get().machine.status === "bootstrapFailed") {
        loadView();
      }
    },
    start: async (inspect) => {
      const { machine } = get();
      if (machine.status !== "idle" || !machine.view.canStart) {
        return;
      }
      set({ machine: { status: "starting", view: machine.view, inspect }, notice: "" });
      try {
        await invoke<void>("start_audio", { inspect });
      } catch (failure) {
        set({
          machine: { status: "faulted", view: machine.view, error: String(failure) },
        });
        flushClose();
      }
    },
    stop: async () => {
      const { machine } = get();
      if (machine.status !== "running") {
        return;
      }
      set({ machine: { status: "stopping", view: machine.view }, notice: "" });
      try {
        await invoke<void>("stop_audio");
      } catch (failure) {
        set({
          machine: { status: "faulted", view: machine.view, error: String(failure) },
        });
        flushClose();
      }
    },
    spectrum: async () => {
      const { machine } = get();
      if (
        machine.status === "bootstrapping" ||
        machine.status === "bootstrapFailed" ||
        machine.status === "starting" ||
        machine.status === "stopping" ||
        machine.status === "closing"
      ) {
        return;
      }
      if (!("view" in machine) || !machine.view.spectrumAvailable) {
        return;
      }
      try {
        await invoke<void>("open_spectrum");
      } catch (failure) {
        set({
          machine: { status: "faulted", view: machine.view, error: String(failure) },
        });
      }
    },
    openSettings: async (section) => {
      try {
        await openSettings(section);
      } catch (failure) {
        const { machine } = get();
        if ("view" in machine) {
          set({
            machine: { status: "faulted", view: machine.view, error: String(failure) },
          });
        }
      }
    },
    openLog: async () => {
      try {
        await invoke<void>("open_log");
      } catch (failure) {
        const { machine } = get();
        if ("view" in machine) {
          set({
            machine: { status: "faulted", view: machine.view, error: String(failure) },
          });
        }
      }
    },
    requestClose: () => {
      const { machine } = get();
      if (machine.status === "closing") {
        return;
      }
      closeRequested = true;
      flushClose();
    },
  };
});
