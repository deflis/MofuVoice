import { connectWindow, connectLogs, type Connection, type WindowMessage } from "../../api";
import { hideLog } from "./api";
import { useLogStore } from "./store";

let windowConnection: Connection | undefined;
let logConnection: Connection | undefined;
let pending: Promise<void> = Promise.resolve();
let request = 0;
const reportError = (error: unknown) => useLogStore.getState().setError(String(error));

function receive(message: WindowMessage) {
  switch (message.type) {
    case "info":
      useLogStore.getState().setInfo(message.info);
      break;
    case "close":
      void closeLog();
      break;
    case "error":
      reportError(message.error);
      break;
    default:
      throw new Error(`ログ画面で不正な通知を受信しました: ${message.type}`);
  }
}
export function connectLogWindow() {
  void windowConnection?.close().catch(reportError);
  void setLogging(false);
  useLogStore.getState().clear();
  windowConnection = connectWindow(receive, reportError);
  return windowConnection;
}
export function setLogging(logging: boolean) {
  const current = ++request;
  useLogStore.getState().setLogging(logging);
  useLogStore.getState().setError("");
  // Stop delivery immediately, even while registration or an older close is pending.
  const closing = logConnection?.close();
  if (!logging) {
    useLogStore.getState().setLog(null);
  }
  pending = pending
    .then(async () => {
      await closing;
      if (current !== request) {
        return undefined;
      }
      logConnection = undefined;
      if (!logging) {
        return undefined;
      }
      logConnection = connectLogs(
        (log) => {
          if (current === request) {
            useLogStore.getState().setLog(log);
          }
        },
        (error) => {
          if (current === request) {
            reportError(error);
          }
        },
      );
      await logConnection.ready;
      return undefined;
    })
    .catch((error: unknown) => {
      if (current === request) {
        useLogStore.getState().setLogging(false);
        useLogStore.getState().setLog(null);
        reportError(error);
      }
    });
  return pending;
}
async function closeLog() {
  await setLogging(false);
  try {
    await hideLog();
  } catch (error) {
    reportError(error);
  }
}
