import { setImmediate } from "node:timers/promises";
import { afterEach, beforeEach, mock } from "node:test";
import { registerHooks } from "node:module";

// Node's type stripping needs explicit extensions for the frontend's bundler imports.
const frontendRoot = new URL("../", import.meta.url).href;
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      context.parentURL?.startsWith(frontendRoot) &&
      specifier.startsWith(".") &&
      !/\.[a-z]+$/.test(specifier)
    ) {
      return nextResolve(`${specifier}.ts`, context);
    }
    return nextResolve(specifier, context);
  },
});

export const settings = {
  eq: {
    enabled: false,
    mode: "simple",
    bands: [
      { frequency: "250", gainDb: "0", q: "0.7" },
      { frequency: "3000", gainDb: "0", q: "0.8" },
      { frequency: "8000", gainDb: "0", q: "0.7" },
    ],
    outputDb: "0",
  },
  device: "ASIO",
  backend: "auto",
  outputDevice: "",
  inputChannel: 1,
  outputChannels: [1],
  frames: "64",
  pitch: "0",
  formantShift: "0",
  preserveFormants: true,
  rumbleFilter: false,
  deesser: false,
  deesserMode: "simple",
  deesserParameters: {
    frequencyHz: "5000",
    thresholdDb: "-24",
    rangeDb: "6",
    attackMs: "0.5",
    releaseMs: "60",
  },
  gate: false,
  gateThresholdMode: "auto",
  gateThresholdDb: "-50",
  gateAttackMs: "1",
  gateReleaseMs: "100",
  level: {
    enabled: false,
    targetDb: "-18",
    maxGainDb: "6",
    floorDb: "-55",
    responseMs: "1200",
    thresholdDb: "-10",
    ratio: "4",
    attackMs: "3",
    releaseMs: "100",
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
  audioBusy: false,
};

export const commands = {};
export const listen = mock.fn();
mock.module("@tauri-apps/api/core", {
  exports: { invoke: (command, args) => commands[command](args) },
});
mock.module("@tauri-apps/api/event", { exports: { listen } });
export const { useMainStore: mainStore } = await import("../windows/main/store.ts");
export const { useSettingsStore: settingsStore } = await import("../windows/settings/store.ts");
export const { useLogStore: logStore } = await import("../windows/log/store.ts");
export const cleanups = [];
export function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  mainStore.setState(mainStore.getInitialState(), true);
  settingsStore.setState(settingsStore.getInitialState(), true);
  logStore.setState(logStore.getInitialState(), true);
  commands.main_view = mock.fn(async () => mainView);
  commands.settings_view = mock.fn(async () => settingsView);
  commands.log_view = mock.fn(async () => ({
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
  }));
  commands.device_choices = mock.fn(async () => ({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  }));
  commands.acknowledge_application_close = mock.fn(async () => true);
  commands.fallback_application_close = mock.fn(async () => {});
  commands.set_logging = mock.fn(async ({ logging }) =>
    logging ? { elapsedSeconds: 0, telemetry: null } : null,
  );
  for (const name of [
    "start_audio",
    "stop_audio",
    "save_settings",
    "open_spectrum",
    "open_settings",
    "open_log",
    "close_settings",
    "close_log",
    "close_application",
  ]) {
    commands[name] = mock.fn(async () => {});
  }
  listen.mock.resetCalls();
  listen.mock.mockImplementation(async () => () => {});
});
afterEach(async () => {
  for (const cleanup of cleanups.splice(0)) {
    cleanup();
  }
  await setImmediate();
});
export function callback(event) {
  return listen.mock.calls.findLast((call) => call.arguments[0] === event).arguments[1];
}
