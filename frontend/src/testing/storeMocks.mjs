import { setImmediate } from "node:timers/promises";
import { afterEach, beforeEach, mock } from "node:test";
import { registerTypeScript } from "./registerTypeScript.mjs";

registerTypeScript();

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
  sampleRate: "48000",
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

const { createMainStore } = await import("../windows/main/store.ts");
const { createSettingsStore } = await import("../windows/settings/store.ts");
const { createLogStore } = await import("../windows/log/store.ts");

export let mainStore, settingsStore, logStore;
export let mainApi, settingsApi, logApi;
export const listen = mock.fn();
const subscribe = (event, receive) => listen(event, ({ payload }) => receive(payload));
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
  listen.mock.resetCalls();
  listen.mock.mockImplementation(async () => () => {});
  mainApi = {
    getView: mock.fn(async () => mainView),
    start: mock.fn(async () => {}),
    stop: mock.fn(async () => {}),
    spectrum: mock.fn(async () => {}),
    openSettings: mock.fn(async () => {}),
    openLog: mock.fn(async () => {}),
    close: mock.fn(async () => {}),
    onPhase: (receive) => subscribe("audio-phase", receive),
    onView: (receive) => subscribe("main-view", receive),
    onClose: (receive) => subscribe("request-close", receive),
  };
  settingsApi = {
    getView: mock.fn(async () => settingsView),
    save: mock.fn(async () => {}),
    deviceChoices: mock.fn(async () => ({
      devices: ["Mic"],
      outputDevices: ["Headphones"],
      audioAvailable: true,
      deviceError: "",
      resolvedBackend: "cpal",
    })),
    deviceSampleRates: mock.fn(async () => ({ input: [48000], output: [48000], common: [48000] })),
    hide: mock.fn(async () => {}),
    closeApplication: mock.fn(async () => {}),
    acknowledgeClose: mock.fn(async () => true),
    fallbackClose: mock.fn(async () => {}),
    onPhase: (receive) => subscribe("audio-phase", receive),
    onClose: (receive) => subscribe("request-close", receive),
    onApplicationClose: (receive) => subscribe("request-application-close", receive),
  };
  logApi = {
    getView: mock.fn(async () => ({
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
    })),
    setLogging: mock.fn(async (logging) =>
      logging ? { elapsedSeconds: 0, telemetry: null } : null,
    ),
    hide: mock.fn(async () => {}),
    onInfo: (receive) => subscribe("log-info", receive),
    onLog: (receive) => subscribe("audio-log", receive),
    onClose: (receive) => subscribe("request-close", receive),
  };
  mainStore = createMainStore(mainApi);
  settingsStore = createSettingsStore(settingsApi);
  logStore = createLogStore(logApi);
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
