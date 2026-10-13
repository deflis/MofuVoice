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
export const bootstrap = {
  settings,
  devices: ["ASIO"],
  outputDevices: [],
  backends: ["auto", "cpal", "asio"],
  levelDefaults: settings.level,
  spectrumAvailable: true,
  audioAvailable: true,
  deviceError: "",
  resolvedBackend: "asio",
};
export const commands = {};
export const listen = mock.fn();
mock.module("@tauri-apps/api/core", {
  exports: { invoke: (command, args) => commands[command](args) },
});
mock.module("@tauri-apps/api/event", { exports: { listen } });
export const { useSettingsStore: settingsStore } = await import("../settingsStore.ts");
export const { useAudioStore: audioStore } = await import("../audioStore.ts");
export const { useAudioLogStore: logStore } = await import("../audioLogStore.ts");
export const { connectApplication } = await import("../applicationConnection.ts");
export let connection;
export function setConnection(value) {
  connection = value;
  return value;
}
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
  settingsStore.setState(settingsStore.getInitialState(), true);
  audioStore.setState(audioStore.getInitialState(), true);
  logStore.setState(logStore.getInitialState(), true);
  commands.bootstrap = mock.fn(async () => bootstrap);
  commands.device_choices = mock.fn(async () => ({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  }));
  commands.audio_status = mock.fn(async () => ({ busy: false, info: { phase: "ready" } }));
  commands.set_logging = mock.fn(async ({ logging }) =>
    logging ? { elapsedSeconds: 0, telemetry: null } : null,
  );
  for (const name of [
    "start_audio",
    "stop_audio",
    "save_settings",
    "open_spectrum",
    "close_application",
  ]) {
    commands[name] = mock.fn(async () => {});
  }
  listen.mock.resetCalls();
  listen.mock.mockImplementation(async () => () => {});
});
afterEach(async () => {
  connection?.disconnect();
  connection = undefined;
  for (const cleanup of cleanups.splice(0)) {
    cleanup();
  }
  await setImmediate();
});
export function callback(event) {
  return listen.mock.calls.find((call) => call.arguments[0] === event).arguments[1];
}
