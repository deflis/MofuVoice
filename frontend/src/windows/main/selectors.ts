import type { MainState } from "./store";
import type { MainSetup } from "../../types";

export function headerLabel(machine: MainState): string {
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
      throw new Error(`未知のメイン状態です: ${JSON.stringify(machine.status)}`);
    }
  }
}

export function canStart(machine: MainState): boolean {
  return (machine.status === "idle" || machine.status === "faulted") && machine.canStart;
}

export function canStop(machine: MainState): boolean {
  return machine.status === "running";
}

export function canOpenSpectrum(machine: MainState): boolean {
  return (
    (machine.status === "idle" || machine.status === "running" || machine.status === "faulted") &&
    machine.spectrumAvailable
  );
}

export function togglePending(machine: MainState): boolean {
  return machine.status === "starting" || machine.status === "stopping";
}

export function setupGuidance(setup: MainSetup): string {
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
