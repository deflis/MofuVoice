import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { afterEach, beforeEach, mock, test } from "node:test";

const settings = {
  device: "ASIO",
  backend: "auto",
  outputDevice: "",
  frames: "64",
  pitch: "0",
  formantShift: "0",
  preserveFormants: true,
  gate: false,
  gateThreshold: "auto",
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
const bootstrap = {
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
const api = {};
const listen = mock.fn();
mock.module("@tauri-apps/api/core", {
  exports: {
    invoke: (command, args) => {
      switch (command) {
        case "bootstrap":
          return api.bootstrap();
        case "audio_status":
          return api.status();
        case "start_audio":
          return api.start(args.settings, args.inspect);
        case "stop_audio":
          return api.stop();
        case "save_settings":
          return api.save(args.settings);
        case "open_spectrum":
          return api.spectrum();
        case "close_application":
          return api.close(args.settings);
        case "device_choices":
          return api.devices(args.backend);
        default:
          throw new Error(`Unexpected command ${command}`);
      }
    },
  },
});
mock.module("@tauri-apps/api/event", { exports: { listen } });
const { useSettingsStore: store } = await import("./settingsStore.ts");
let disconnect;

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  store.setState(store.getInitialState(), true);
  api.bootstrap = mock.fn(async () => bootstrap);
  api.devices = mock.fn(async () => ({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  }));
  api.status = mock.fn(async () => ({ busy: false, status: "idle" }));
  for (const action of ["start", "stop", "save", "spectrum", "close"]) {
    api[action] = mock.fn(async () => {});
  }
  listen.mock.resetCalls();
  listen.mock.mockImplementation(async () => () => {});
});
afterEach(() => {
  disconnect?.();
  disconnect = undefined;
});

test("close during bootstrap waits for settings and sends only one shutdown", async () => {
  const loading = deferred();
  api.bootstrap.mock.mockImplementation(() => loading.promise);
  disconnect = store.getState().connect();
  await store.getState().close();
  await store.getState().close();
  assert.equal(api.close.mock.callCount(), 0);
  loading.resolve(bootstrap);
  await setImmediate();
  assert.equal(api.close.mock.callCount(), 1);
  assert.equal(api.close.mock.calls[0].arguments[0], settings);
});

test("close during a command runs after completion even when the command fails", async () => {
  for (const failed of [false, true]) {
    store.setState({ settings, closing: false });
    const saving = deferred();
    api.save.mock.mockImplementation(() => saving.promise);
    const operation = store.getState().save();
    await store.getState().close();
    const before = api.close.mock.callCount();
    if (failed) {
      saving.reject(new Error("save failed"));
    } else {
      saving.resolve();
    }
    await operation;
    assert.equal(api.close.mock.callCount(), before + 1);
    assert.equal(store.getState().pending, false);
  }
});

test("bootstrap failure exposes retry state and reconnect loads settings", async () => {
  api.bootstrap.mock.mockImplementation(async () => {
    throw new Error("bootstrap failed");
  });
  disconnect = store.getState().connect();
  await setImmediate();
  assert.equal(store.getState().loadFailed, true);
  assert.match(store.getState().error, /bootstrap failed/);
  assert.equal(store.getState().settings, undefined);
  disconnect();
  api.bootstrap.mock.mockImplementation(async () => bootstrap);
  disconnect = store.getState().connect();
  assert.equal(store.getState().loadFailed, false);
  await setImmediate();
  assert.equal(store.getState().settings, settings);
  assert.equal(store.getState().error, "");
});

test("setup-cleanup-setup ignores old responses and removes late subscriptions", async () => {
  const loading = deferred();
  const status = deferred();
  const subscription = deferred();
  const remove = mock.fn();
  api.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  api.status.mock.mockImplementationOnce(() => status.promise);
  listen.mock.mockImplementationOnce(() => subscription.promise);
  const cleanup = store.getState().connect();
  const obsoleteClose = listen.mock.calls[0].arguments[1];
  cleanup();
  disconnect = store.getState().connect();
  await setImmediate();
  loading.resolve({ ...bootstrap, settings: { ...settings, pitch: "12" } });
  status.resolve({ busy: true, status: "obsolete" });
  subscription.resolve(remove);
  obsoleteClose();
  await setImmediate();
  assert.equal(store.getState().settings.pitch, "0");
  assert.equal(store.getState().snapshot.status, "idle");
  assert.equal(remove.mock.callCount(), 1);
  assert.equal(api.close.mock.callCount(), 0);
});

test("failed shutdown reports validation error and allows draft correction", async () => {
  store.setState({ settings: { ...settings, pitch: "" } });
  api.close.mock.mockImplementation(async () => {
    throw new Error("invalid pitch");
  });
  await store.getState().close();
  assert.equal(store.getState().closing, false);
  assert.match(store.getState().error, /invalid pitch/);
  store.getState().update("pitch", "3");
  api.close.mock.mockImplementation(async () => {});
  await store.getState().close();
  assert.equal(api.close.mock.calls[1].arguments[0].pitch, "3");
});

test("failed bootstrap cancels deferred close before an explicit retry", async () => {
  const loading = deferred();
  api.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  disconnect = store.getState().connect();
  await store.getState().close();
  loading.reject(new Error("bootstrap failed"));
  await setImmediate();
  disconnect();
  disconnect = store.getState().connect();
  await setImmediate();
  assert.equal(store.getState().settings, settings);
  assert.equal(api.close.mock.callCount(), 0);
});

test("cleanup cancels polling and ignores failures from obsolete connections", async (context) => {
  context.mock.timers.enable({ apis: ["setTimeout"] });
  const loading = deferred();
  const status = deferred();
  api.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  api.status.mock.mockImplementationOnce(() => status.promise);
  const cleanup = store.getState().connect();
  cleanup();
  disconnect = store.getState().connect();
  await setImmediate();
  loading.reject(new Error("obsolete bootstrap failure"));
  status.reject(new Error("obsolete status failure"));
  await setImmediate();
  assert.equal(store.getState().error, "");
  assert.equal(store.getState().loadFailed, false);
  context.mock.timers.tick(250);
  await setImmediate();
  assert.equal(api.status.mock.callCount(), 3);
  disconnect();
  context.mock.timers.tick(1000);
  await setImmediate();
  assert.equal(api.status.mock.callCount(), 3);
});

test("backend switching refreshes device choices and resets both endpoints", async () => {
  store.setState(bootstrap);
  const loading = deferred();
  api.devices.mock.mockImplementation(() => loading.promise);
  const switching = store.getState().updateBackend("cpal");
  assert.equal(store.getState().pending, true);
  assert.equal(store.getState().settings.backend, "cpal");
  assert.equal(store.getState().settings.device, "");
  assert.equal(store.getState().settings.outputDevice, "");
  assert.equal(store.getState().audioAvailable, false);
  loading.resolve({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  });
  await switching;
  assert.deepEqual(store.getState().devices, ["Mic"]);
  assert.deepEqual(store.getState().outputDevices, ["Headphones"]);
  assert.equal(store.getState().resolvedBackend, "cpal");
  assert.equal(store.getState().pending, false);
});
