import { connectWindow, type Connection, type WindowMessage } from "../../api";
import { mainApi } from "./api";
import { useMainStore, type MainEvent } from "./store";
import { canStart, canStop, canOpenSpectrum } from "./selectors";
import type { SettingsSection } from "../settings/navigation";

let connection: Connection | undefined;
let closing: Promise<void> | undefined;
const send = (event: MainEvent) => useMainStore.getState().send(event);
const reportError = (error: unknown) => send({ type: "error", error: String(error) });

async function closeWhenReady() {
  if (useMainStore.getState().status !== "closing" || closing) {
    return;
  }
  const owner = connection;
  closing = mainApi.close();
  try {
    await closing;
  } catch (error) {
    if (owner === connection) {
      closing = undefined;
      send({ type: "closeFailed", error: String(error) });
    }
  }
}
function receive(message: WindowMessage) {
  switch (message.type) {
    case "main":
      send({ type: "view", view: message.view });
      break;
    case "phase":
      send({ type: "phase", phase: message.phase });
      break;
    case "close":
      send({ type: "close" });
      break;
    case "error":
      reportError(message.error);
      break;
    default:
      throw new Error(`メイン画面で不正な通知を受信しました: ${message.type}`);
  }
  void closeWhenReady();
}
export function activeMainConnection() {
  return connection;
}
export function connectMainWindow() {
  const previous = connection?.close();
  closing = undefined;
  send({ type: "connect" });
  connection = connectWindow(receive, (error) =>
    send({ type: "connectionFailed", error: String(error) }),
  );
  void previous?.catch((error: unknown) => {
    console.error(error);
  });
  return connection;
}
async function runAction(action: () => Promise<void>) {
  const owner = connection;
  try {
    await action();
  } catch (error) {
    if (owner === connection) {
      reportError(error);
    }
  }
}
export async function startAudio(inspect: boolean) {
  if (!canStart(useMainStore.getState())) {
    return;
  }
  send({ type: "start" });
  const pending = useMainStore.getState();
  try {
    await mainApi.start(inspect);
  } catch (error) {
    if (
      useMainStore.getState().commandId === pending.commandId &&
      useMainStore.getState().status === pending.status
    ) {
      send({ type: "commandFailed", error: String(error) });
    }
  }
  await closeWhenReady();
}
export async function stopAudio() {
  if (!canStop(useMainStore.getState())) {
    return;
  }
  send({ type: "stop" });
  const pending = useMainStore.getState();
  try {
    await mainApi.stop();
  } catch (error) {
    if (
      useMainStore.getState().commandId === pending.commandId &&
      useMainStore.getState().status === pending.status
    ) {
      send({ type: "commandFailed", error: String(error) });
    }
  }
  await closeWhenReady();
}
export function openSpectrum() {
  if (!canOpenSpectrum(useMainStore.getState())) {
    return Promise.resolve();
  }
  return runAction(mainApi.spectrum);
}
export const openSettings = (section: SettingsSection) =>
  runAction(() => mainApi.openSettings(section));
export const openLog = () => runAction(mainApi.openLog);
