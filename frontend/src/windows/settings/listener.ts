import { create } from "zustand";
import { connectWindow, type Connection, type WindowMessage } from "../../api";
import type { DeviceChoices, LevelSettings } from "../../types";
import { settingsApi } from "./api";
import { useSettingsStore } from "./store";
import * as v from "valibot";
import { devicesSchema } from "./devices/schema";
import { loadSettings } from "./values";

type ConnectionState = DeviceChoices & {
  status: "loading" | "loadFailed" | "ready" | "saving" | "switchingBackend" | "closing";
  levelDefaults: LevelSettings | undefined;
  backends: string[];
  audioBusy: boolean;
  error: string;
  notice: string;
};
const initial: ConnectionState = {
  status: "loading",
  devices: [],
  outputDevices: [],
  backends: [],
  levelDefaults: undefined,
  deviceError: "",
  resolvedBackend: "",
  audioAvailable: false,
  audioBusy: true,
  error: "",
  notice: "",
};
// Connection metadata is separate from the editable, flat settings values.
export const useSettingsConnection = create<ConnectionState>(() => ({ ...initial }));
export function canEditSettings(state: Pick<ConnectionState, "status" | "audioBusy">) {
  return state.status === "ready" && !state.audioBusy;
}
type Session = {
  connection?: Connection;
  operation?: Promise<void>;
  loaded: boolean;
  closing: boolean;
  closeTarget?: "window" | "application";
};
let session: Session | undefined;
const reportError = (error: unknown) => useSettingsConnection.setState({ error: String(error) });

async function cancelClose(owner: Session) {
  if (owner.closeTarget === "application") {
    await settingsApi.cancelApplicationClose();
  }
  owner.closeTarget = undefined;
}
async function flushClose(owner: Session, saved: boolean) {
  if (owner !== session || !owner.closeTarget || owner.operation || owner.closing) {
    return;
  }
  if (!owner.loaded) {
    if (useSettingsConnection.getState().status !== "loadFailed") {
      return;
    }
    try {
      if (owner.closeTarget === "application") {
        // Snapshot never loaded; exit with the last persisted backend settings.
        await settingsApi.closeApplication();
        if (owner === session) {
          owner.closeTarget = undefined;
        }
      } else {
        owner.closeTarget = undefined;
        await settingsApi.hide();
      }
    } catch (error) {
      if (owner === session) {
        try {
          await cancelClose(owner);
        } catch (failure) {
          reportError(failure);
        }
        reportError(error);
      }
    }
    return;
  }
  owner.closing = true;
  useSettingsConnection.setState({ status: "closing", error: "", notice: "" });
  try {
    if (!saved) {
      await useSettingsStore.getState().save();
    }
    if (owner !== session) {
      return;
    }
    if (owner.closeTarget === "window") {
      await settingsApi.hide();
    }
    if (owner !== session) {
      return;
    }
    // An application close may have arrived while the window was being hidden.
    if (owner.closeTarget === "application") {
      await settingsApi.closeApplication();
    }
    if (owner === session) {
      owner.closeTarget = undefined;
      useSettingsConnection.setState({ status: "ready", notice: "設定を保存しました。" });
    }
  } catch (error) {
    if (owner !== session) {
      return;
    }
    try {
      await cancelClose(owner);
    } catch (failure) {
      reportError(failure);
    }
    useSettingsConnection.setState({ status: "ready", error: String(error) });
  } finally {
    owner.closing = false;
  }
}
function receive(owner: Session, message: WindowMessage) {
  if (owner !== session) {
    return;
  }
  switch (message.type) {
    case "phase":
      useSettingsConnection.setState({ audioBusy: message.phase.busy });
      break;
    case "applicationClose":
      owner.closeTarget = "application";
      void flushClose(owner, false);
      break;
    case "close":
      owner.closeTarget ??= "window";
      void flushClose(owner, false);
      break;
    case "error":
      reportError(message.error);
      break;
    default:
      throw new Error(`設定画面で不正な通知を受信しました: ${message.type}`);
  }
}
async function load(owner: Session) {
  try {
    await owner.connection?.ready;
    if (owner !== session) {
      return;
    }
    const { settings, ...metadata } = await settingsApi.getView();
    if (owner !== session) {
      return;
    }
    useSettingsStore.setState((state) => loadSettings(state, settings));
    owner.loaded = true;
    useSettingsConnection.setState({ ...metadata, status: "ready", error: "" });
  } catch (error) {
    if (owner !== session) {
      return;
    }
    useSettingsConnection.setState({ status: "loadFailed", error: String(error) });
  }
  await flushClose(owner, false);
}
export function activeSettingsConnection() {
  return session?.connection;
}
export function connectSettingsWindow() {
  const previous = session?.connection?.close();
  const owner: Session = { loaded: false, closing: false };
  session = owner;
  useSettingsConnection.setState({ ...initial });
  owner.connection = connectWindow(
    (message) => receive(owner, message),
    (error) => {
      if (owner === session) {
        useSettingsConnection.setState({ status: "loadFailed", error: String(error) });
      }
    },
  );
  void previous?.catch((error: unknown) => {
    if (owner === session) {
      console.error(error);
    }
  });
  void load(owner);
  return owner.connection;
}
async function runOperation(status: "saving" | "switchingBackend", action: () => Promise<void>) {
  const owner = session;
  if (
    !owner ||
    owner.operation ||
    owner.closing ||
    !canEditSettings(useSettingsConnection.getState())
  ) {
    return;
  }
  useSettingsConnection.setState({ status, error: "", notice: "" });
  const operation = action();
  owner.operation = operation;
  let succeeded = false;
  try {
    await operation;
    succeeded = true;
    if (owner === session) {
      useSettingsConnection.setState({
        status: "ready",
        notice: status === "saving" ? "設定を保存しました。" : "",
      });
    }
  } catch (error) {
    if (owner === session) {
      try {
        await cancelClose(owner);
      } catch (failure) {
        reportError(failure);
      }
      useSettingsConnection.setState({ status: "ready", error: String(error) });
    }
  } finally {
    owner.operation = undefined;
  }
  if (succeeded) {
    await flushClose(owner, status === "saving");
  }
}
export const saveSettings = () => runOperation("saving", () => useSettingsStore.getState().save());
export function changeBackend(backend: string) {
  return runOperation("switchingBackend", async () => {
    const owner = session;
    const values = useSettingsStore.getState().devices;
    values.setBackend(v.parse(devicesSchema.entries.backend, backend));
    values.setDevice("");
    values.setOutputDevice("");
    useSettingsConnection.setState({
      devices: [],
      outputDevices: [],
      audioAvailable: false,
      resolvedBackend: "",
      deviceError: "",
    });
    const choices = await settingsApi.deviceChoices(backend);
    if (owner === session) {
      useSettingsConnection.setState(choices);
    }
  });
}
