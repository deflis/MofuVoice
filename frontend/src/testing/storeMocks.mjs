import { setImmediate } from "node:timers/promises";
import { afterEach, beforeEach, mock } from "node:test";
import { registerTypeScript } from "./registerTypeScript.mjs";
registerTypeScript();

export const settings = {
  devices: {
    backend: "auto",
    device: "ASIO",
    outputDevice: "",
    inputChannel: 1,
    outputChannels: [1],
    frames: 64,
    sampleRate: 48000,
  },
  voice: {
    pitchUnit: "semitones",
    pitchAlgorithm: "phase-locked",
    psolaAnalysisWindow: 2048,
    psolaSynthesisWindow: 512,
    analysisWindow: 2048,
    synthesisWindow: 512,
    pitch: 0,
    formantShift: 0,
    preserveFormants: true,
  },
  noise: {
    rumbleFilter: false,
    enabled: false,
    thresholdMode: "auto",
    thresholdDb: -50,
    attackMs: 1,
    releaseMs: 100,
  },
  deesser: {
    enabled: false,
    mode: "simple",
    frequencyHz: 5000,
    thresholdDb: -24,
    rangeDb: 6,
    attackMs: 0.5,
    releaseMs: 60,
  },
  eq: {
    enabled: false,
    mode: "simple",
    bands: [
      {
        frequency: 250,
        gainDb: 0,
        q: 0.7,
      },
      {
        frequency: 3000,
        gainDb: 0,
        q: 0.8,
      },
      {
        frequency: 8000,
        gainDb: 0,
        q: 0.7,
      },
    ],
    outputDb: 0,
  },
  level: {
    enabled: false,
    compressorEnabled: false,
    targetDb: -18,
    maxGainDb: 6,
    floorDb: -55,
    responseMs: 1200,
    thresholdDb: -10,
    ratio: 4,
    attackMs: 3,
    releaseMs: 100,
  },
};
export const mainView = {
  canStart: true,
  spectrumAvailable: true,
  setup: {
    audioAvailable: true,
    resolvedBackend: "asio",
    inputConfigured: true,
    outputConfigured: true,
    rememberedDeviceMissing: false,
  },
  phase: { phase: "ready", error: null, busy: false },
};

export const settingsView = {
  settings,
  devices: ["ASIO"],
  outputDevices: [],
  backends: ["auto", "cpal", "asio"],
  levelDefaults: settings.level,
  audioAvailable: true,
  deviceError: "",
  resolvedBackend: "asio",
};

export const handlers = new Map();
export const channels = new Map();
export const connections = [];
let windowKind = "main";
export class Channel {
  constructor(receive) {
    Object.defineProperty(this, "onmessage", { value: receive, writable: true });
  }
  send(message) {
    this.onmessage(message);
  }
}
export const info = {
  phase: "ready",
  error: null,
  device: null,
  outputDevice: null,
  backend: null,
  sampleRate: null,
  inputs: null,
  outputs: null,
  frames: null,
  periodMs: null,
  dspMs: null,
  inputLatencyMs: null,
  outputLatencyMs: null,
};
export const invoke = mock.fn(async (command, args) => {
  if (handlers.has(command)) {
    return handlers.get(command)(args);
  }
  if (command === "connect_window") {
    channels.set(windowKind, args.channel);
    let message = { type: "info", info };
    if (windowKind === "main") {
      message = { type: "main", view: mainView };
    }
    if (windowKind === "settings") {
      message = { type: "phase", phase: mainView.phase };
    }
    args.channel.onmessage(message);
  }
  if (command === "connect_logs") {
    channels.set("logs", args.channel);
    args.channel.onmessage({ elapsedSeconds: 0, telemetry: null });
  }
  if (command === "settings_view") {
    return settingsView;
  }
  if (command === "device_choices") {
    return {
      devices: ["Mic"],
      outputDevices: ["Headphones"],
      audioAvailable: true,
      deviceError: "",
      resolvedBackend: "cpal",
    };
  }
  return undefined;
});
mock.module("@tauri-apps/api/core", { exports: { invoke, Channel } });
export const { useMainStore: mainStore } = await import("../windows/main/store.ts");
export const { useSettingsStore: settingsStore } = await import("../windows/settings/store.ts");
export const { useLogStore: logStore } = await import("../windows/log/store.ts");
export const main = await import("../windows/main/listener.ts");
export const settingsListener = await import("../windows/settings/listener.ts");
export const log = await import("../windows/log/listener.ts");
const { loadSettings } = await import("../windows/settings/values.ts");
const { settingsSchema } = await import("../windows/settings/schema.ts");
const { parse } = await import("valibot");
export const toSettings = (state) => parse(settingsSchema, state);
export const tick = () => setImmediate();
export const deferred = () => Promise.withResolvers();
export const calls = (command) => invoke.mock.calls.filter((call) => call.arguments[0] === command);
export function push(message, kind = windowKind) {
  channels.get(kind).onmessage(message);
}
export function connect(kind) {
  windowKind = kind;
  let connection;
  if (kind === "main") {
    connection = main.connectMainWindow();
  } else if (kind === "settings") {
    connection = settingsListener.connectSettingsWindow();
  } else {
    connection = log.connectLogWindow();
  }
  connections.push(connection);
  return connection;
}
beforeEach(() => {
  handlers.clear();
  channels.clear();
  invoke.mock.resetCalls();
  mainStore.getState().send({ type: "connect" });
  settingsStore.setState((state) => loadSettings(state, settings));
  logStore.getState().clear();
});
afterEach(async () => {
  await log.setLogging(false);
  await Promise.all(connections.splice(0).map((connection) => connection.close()));
  await tick();
});
