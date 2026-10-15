import { mainApi, type MainApi } from "./api";
import { create } from "zustand";
import type { AudioPhase, MainView } from "../../types";
import type { SettingsSection } from "../settings/navigation";

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
  error: string;
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

function isSettledPhase(phase: string): boolean {
  return phase === "ready" || phase === "stopped" || phase === "inspected";
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
    if (isSettledPhase(phase.phase)) {
      // Worker may still be joining after stopped/inspected; wait for busy=false.
      return phase.busy ? machine : { status: "idle", view: machine.view };
    }
  }
  if (machine.status === "stopping") {
    if (isSettledPhase(phase.phase) && !phase.busy) {
      return { status: "idle", view: machine.view };
    }
    return machine;
  }
  if (phase.phase === "running" || phase.phase === "opening") {
    return { status: "running", view: machine.view };
  }
  if (isSettledPhase(phase.phase)) {
    if (phase.busy) {
      return { status: "stopping", view: machine.view };
    }
    return { status: "idle", view: machine.view };
  }
  return {
    status: "faulted",
    view: machine.view,
    error: `不正な状態遷移です（${machine.status} ← ${phase.phase}）。`,
  };
}

function reportActionError(set: (partial: Partial<MainStore>) => void, failure: unknown) {
  set({ error: String(failure) });
}

export function createMainStore(api: MainApi = mainApi) {
  return create<MainStore>()((set, get) => {
    let lifetime = 0;
    let disconnectCurrent: (() => void) | undefined;
    let closeRequested = false;
    let loadGeneration = 0;
    let pendingPhase: AudioPhase | undefined;
    let pendingView: MainView | undefined;
    let closingPhase: AudioPhase | undefined;
    let commandGeneration = 0;

    const loadView = () => {
      const generation = ++loadGeneration;
      pendingPhase = undefined;
      pendingView = undefined;
      set({ machine: { status: "bootstrapping" }, error: "", notice: "" });
      void api
        .getView()
        .then((view) => {
          if (generation !== loadGeneration) {
            return undefined;
          }
          const currentView = pendingView ?? view;
          const phase = pendingPhase ?? currentView.phase;
          pendingPhase = undefined;
          pendingView = undefined;
          set({ machine: applyPhase({ status: "idle", view: currentView }, phase) });
          flushClose();
          return undefined;
        })
        .catch((failure: unknown) => {
          if (generation !== loadGeneration) {
            return undefined;
          }
          pendingPhase = undefined;
          set({ machine: { status: "bootstrapFailed", error: String(failure) } });
          closeRequested = false;
          return undefined;
        });
    };

    const beginClose = () => {
      closeRequested = false;
      const previous = get().machine;
      const generation = lifetime;
      closingPhase = undefined;
      set({ machine: { status: "closing" }, notice: "", error: "" });
      void api.close().catch((failure: unknown) => {
        if (generation !== lifetime || get().machine.status !== "closing") {
          return;
        }
        // Preserve the prior audio machine so Stop remains available if the
        // backend session is still running (e.g. settings save failed first).
        set({
          machine: closingPhase ? applyPhase(previous, closingPhase) : previous,
          error: String(failure),
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
      error: "",
      notice: "",
      mount: () => {
        disconnectCurrent?.();
        const mounted = ++lifetime;
        let disposed = false;
        closeRequested = false;
        pendingPhase = undefined;

        const reportListenError = (failure: unknown) => {
          if (!disposed) {
            reportActionError(set, failure);
          }
        };

        const unlistenPhase = api.onPhase((payload) => {
          if (disposed) {
            return;
          }
          if (get().machine.status === "bootstrapping") {
            pendingPhase = payload;
            return;
          }
          if (get().machine.status === "closing") {
            closingPhase = payload;
          }
          set((state) => ({ machine: applyPhase(state.machine, payload) }));
          flushClose();
        });
        void unlistenPhase.catch(reportListenError);

        const unlistenView = api.onView((payload) => {
          if (disposed) {
            return;
          }
          if (get().machine.status === "bootstrapping") {
            pendingView = payload;
            return;
          }
          set((state) => ({ machine: withView(state.machine, payload) }));
        });
        void unlistenView.catch(reportListenError);

        const unlistenClose = api.onClose(() => {
          if (!disposed) {
            get().requestClose();
          }
        });
        void unlistenClose.catch(reportListenError);

        loadView();

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
          pendingPhase = undefined;
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
        return disconnectCurrent;
      },
      retry: () => {
        if (get().machine.status === "bootstrapFailed") {
          loadView();
        }
      },
      start: async (inspect) => {
        const generation = lifetime;
        const { machine } = get();
        if ((machine.status !== "idle" && machine.status !== "faulted") || !machine.view.canStart) {
          return;
        }
        const command = ++commandGeneration;
        set({
          machine: { status: "starting", view: machine.view, inspect },
          error: "",
          notice: "",
        });
        try {
          await api.start(inspect);
        } catch (failure) {
          const current = get().machine;
          if (
            generation !== lifetime ||
            command !== commandGeneration ||
            current.status !== "starting"
          ) {
            return;
          }
          set({ machine: { status: "faulted", view: current.view, error: String(failure) } });
          flushClose();
        }
      },
      stop: async () => {
        const generation = lifetime;
        const { machine } = get();
        if (machine.status !== "running") {
          return;
        }
        const command = ++commandGeneration;
        set({ machine: { status: "stopping", view: machine.view }, error: "", notice: "" });
        try {
          await api.stop();
        } catch (failure) {
          const current = get().machine;
          if (
            generation !== lifetime ||
            command !== commandGeneration ||
            current.status !== "stopping"
          ) {
            return;
          }
          set({ machine: { status: "faulted", view: current.view, error: String(failure) } });
          flushClose();
        }
      },
      spectrum: async () => {
        const generation = lifetime;
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
          await api.spectrum();
        } catch (failure) {
          if (generation === lifetime) {
            reportActionError(set, failure);
          }
        }
      },
      openSettings: async (section) => {
        const generation = lifetime;
        try {
          await api.openSettings(section);
        } catch (failure) {
          if (generation === lifetime) {
            reportActionError(set, failure);
          }
        }
      },
      openLog: async () => {
        const generation = lifetime;
        try {
          await api.openLog();
        } catch (failure) {
          if (generation === lifetime) {
            reportActionError(set, failure);
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
}

export const useMainStore = createMainStore();
