import { create } from "zustand";
import type { AudioPhase, MainView, MainSetup } from "../../types";

export type MainStatus =
  | "bootstrapping"
  | "bootstrapFailed"
  | "idle"
  | "starting"
  | "running"
  | "stopping"
  | "faulted"
  | "closing";
export type MainState = MainSetup & {
  status: MainStatus;
  canStart: boolean;
  spectrumAvailable: boolean;
  audioPhase: string;
  audioBusy: boolean;
  audioError: string | null;
  error: string;
  closeRequested: boolean;
  commandId: number;
};
export type MainEvent =
  | { type: "connect" }
  | { type: "view"; view: MainView }
  | { type: "phase"; phase: AudioPhase }
  | { type: "start" }
  | { type: "stop" }
  | { type: "close" }
  | { type: "closeFailed"; error: string }
  | { type: "commandFailed"; error: string }
  | { type: "connectionFailed"; error: string }
  | { type: "error"; error: string };

const initial: MainState = {
  status: "bootstrapping",
  canStart: false,
  spectrumAvailable: false,
  audioAvailable: false,
  resolvedBackend: "",
  inputConfigured: false,
  outputConfigured: false,
  rememberedDeviceMissing: false,
  audioPhase: "ready",
  audioBusy: false,
  audioError: null,
  error: "",
  closeRequested: false,
  commandId: 0,
};
function phaseStatus(status: MainStatus, phase: AudioPhase): MainStatus {
  if (phase.phase === "error") {
    return "faulted";
  }
  if (status === "starting" && phase.phase === "opening") {
    return "starting";
  }
  const settled =
    phase.phase === "ready" || phase.phase === "stopped" || phase.phase === "inspected";
  if (status === "stopping") {
    return settled && !phase.busy ? "idle" : "stopping";
  }
  if (settled) {
    if (!phase.busy) {
      return "idle";
    }
    return status === "starting" ? "starting" : "stopping";
  }
  if (phase.phase === "running" || phase.phase === "opening") {
    return "running";
  }
  return "faulted";
}
function acceptPhase(state: MainState, phase: AudioPhase): MainState {
  const status = state.status === "closing" ? "closing" : phaseStatus(state.status, phase);
  let error = state.error;
  if (status === "faulted") {
    error = phase.error ?? `不正な音声状態です（${phase.phase}）。`;
  } else if (state.status === "faulted") {
    error = "";
  }
  return {
    ...state,
    audioPhase: phase.phase,
    audioBusy: phase.busy,
    audioError: phase.error,
    status,
    error,
  };
}
export function transition(state: MainState, event: MainEvent): MainState {
  let next = state;
  switch (event.type) {
    case "connect":
      return { ...initial, commandId: state.commandId + 1 };
    case "connectionFailed":
      return { ...state, status: "bootstrapFailed", error: event.error, closeRequested: false };
    case "error":
      return { ...state, error: event.error };
    case "view":
      next = acceptPhase(
        {
          ...state,
          ...event.view.setup,
          canStart: event.view.canStart,
          spectrumAvailable: event.view.spectrumAvailable,
        },
        event.view.phase,
      );
      break;
    case "phase":
      if (state.status === "bootstrapping" || state.status === "bootstrapFailed") {
        return state;
      }
      next = acceptPhase(state, event.phase);
      break;
    case "start":
      if ((state.status !== "idle" && state.status !== "faulted") || !state.canStart) {
        return state;
      }
      return { ...state, status: "starting", error: "", commandId: state.commandId + 1 };
    case "stop":
      return state.status === "running"
        ? { ...state, status: "stopping", error: "", commandId: state.commandId + 1 }
        : state;
    case "close":
      next = { ...state, closeRequested: true };
      break;
    case "closeFailed":
      return state.status === "closing"
        ? {
            ...state,
            status: phaseStatus("idle", {
              phase: state.audioPhase,
              busy: state.audioBusy,
              error: state.audioError,
            }),
            error: event.error,
            closeRequested: false,
          }
        : state;
    case "commandFailed":
      if (state.status !== "starting" && state.status !== "stopping") {
        return state;
      }
      next = { ...state, status: "faulted", error: event.error };
      break;
  }
  if (
    next.closeRequested &&
    (next.status === "idle" || next.status === "running" || next.status === "faulted")
  ) {
    return { ...next, status: "closing", closeRequested: false };
  }
  return next;
}
type MainStore = MainState & { send: (event: MainEvent) => void };
export const useMainStore = create<MainStore>()((set) => ({
  ...initial,
  send: (event) => set((state) => transition(state, event)),
}));
