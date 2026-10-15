import type { MainMachine } from "./store";
import type { MainView } from "../../types";

export function headerLabel(machine: MainMachine): string {
  switch (machine.status) {
    case "bootstrapping":
      return "読み込み中";
    case "bootstrapFailed":
      return "読み込み失敗";
    case "idle":
      return "停止中";
    case "starting":
      return "デバイスを起動中";
    case "running":
      return "音声処理中";
    case "stopping":
      return "停止中";
    case "faulted":
      return "エラー";
    case "closing":
      return "終了中";
    default: {
      const unexpected: never = machine;
      throw new Error(`未知のメイン状態です: ${JSON.stringify(unexpected)}`);
    }
  }
}

export function canStart(machine: MainMachine): boolean {
  return (machine.status === "idle" || machine.status === "faulted") && machine.view.canStart;
}

export function canStop(machine: MainMachine): boolean {
  return machine.status === "running";
}

export function canOpenSpectrum(machine: MainMachine): boolean {
  return (
    (machine.status === "idle" || machine.status === "running" || machine.status === "faulted") &&
    machine.view.spectrumAvailable
  );
}

export function togglePending(machine: MainMachine): boolean {
  return machine.status === "starting" || machine.status === "stopping";
}

export function setupGuidance(view: MainView): string {
  const { setup } = view;
  if (!setup.audioAvailable) {
    return "使える音声機器が見つかりません。接続と設定を確認してください。";
  }
  if (!setup.inputConfigured) {
    return "最初に、入力・出力デバイスを設定してください。";
  }
  if (setup.rememberedDeviceMissing) {
    return "前回使った音声機器が見つかりません。入力・出力デバイスを確認してください。";
  }
  return "";
}

export function machineError(machine: MainMachine): string {
  if (machine.status === "bootstrapFailed" || machine.status === "faulted") {
    return machine.error;
  }
  return "";
}
