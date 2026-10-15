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

export const useMainStore = create<MainStore>()((set, get) => {
  let closeRequested = false;
  let loadGeneration = 0;

  const loadView = () => {
    const generation = ++loadGeneration;
    set({ machine: { status: "bootstrapping" }, error: "", notice: "" });
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
    const previous = get().machine;
    set({ machine: { status: "closing" }, notice: "", error: "" });
    void invoke<void>("close_application").catch((failure: unknown) => {
      // Preserve the prior audio machine so Stop remains available if the
      // backend session is still running (e.g. settings save failed first).
      set({ machine: previous, error: String(failure) });
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
      let disposed = false;
      closeRequested = false;
      loadView();

      const reportListenError = (failure: unknown) => {
        if (!disposed) {
          reportActionError(set, failure);
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
      if ((machine.status !== "idle" && machine.status !== "faulted") || !machine.view.canStart) {
        return;
      }
      set({
        machine: { status: "starting", view: machine.view, inspect },
        error: "",
        notice: "",
      });
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
      set({ machine: { status: "stopping", view: machine.view }, error: "", notice: "" });
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
        reportActionError(set, failure);
      }
    },
    openSettings: async (section) => {
      try {
        await openSettings(section);
      } catch (failure) {
        reportActionError(set, failure);
      }
    },
    openLog: async () => {
      try {
        await invoke<void>("open_log");
      } catch (failure) {
        reportActionError(set, failure);
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
