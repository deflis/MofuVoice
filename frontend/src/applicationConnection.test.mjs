import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { mock, test } from "node:test";
const {
  settings,
  bootstrap,
  commands,
  listen,
  settingsStore,
  audioStore,
  connectApplication,
  setConnection,
  deferred,
  callback,
} = await import("./testing/tauriMocks.mjs");

let connection;

test("close during bootstrap waits for settings and sends only one shutdown", async () => {
  const loading = deferred();
  commands.bootstrap.mock.mockImplementation(() => loading.promise);
  connection = setConnection(connectApplication());
  connection.requestClose();
  connection.requestClose();
  assert.equal(commands.close_application.mock.callCount(), 0);
  loading.resolve(bootstrap);
  await setImmediate();
  assert.equal(commands.close_application.mock.callCount(), 1);
  assert.equal(commands.close_application.mock.calls[0].arguments[0].settings, settings);
});
test("close during save waits for completion including save failure", async () => {
  connection = setConnection(connectApplication());
  await setImmediate();
  for (const failed of [false, true]) {
    audioStore.setState({ closing: false });
    const saving = deferred();
    commands.save_settings.mock.mockImplementation(() => saving.promise);
    const operation = settingsStore.getState().save();
    connection.requestClose();
    const before = commands.close_application.mock.callCount();
    if (failed) {
      saving.reject(new Error("save failed"));
    } else {
      saving.resolve();
    }
    await operation;
    assert.equal(commands.close_application.mock.callCount(), before + 1);
    assert.equal(settingsStore.getState().saving, false);
  }
});
test("close during audio command waits for completion", async () => {
  connection = setConnection(connectApplication());
  await setImmediate();
  const starting = deferred();
  commands.start_audio.mock.mockImplementation(() => starting.promise);
  const operation = audioStore.getState().start(settings, false);
  callback("request-close")();
  assert.equal(commands.close_application.mock.callCount(), 0);
  starting.resolve();
  await operation;
  assert.equal(commands.close_application.mock.callCount(), 1);
});
test("bootstrap failure exposes retry state and cancels deferred close", async () => {
  const loading = deferred();
  commands.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  connection = setConnection(connectApplication());
  connection.requestClose();
  loading.reject(new Error("bootstrap failed"));
  await setImmediate();
  assert.equal(settingsStore.getState().loadFailed, true);
  assert.match(settingsStore.getState().error, /bootstrap failed/);
  connection.disconnect();
  connection = setConnection(connectApplication());
  assert.equal(settingsStore.getState().loadFailed, false);
  await setImmediate();
  assert.equal(settingsStore.getState().settings, settings);
  assert.equal(commands.close_application.mock.callCount(), 0);
});
test("setup-cleanup-setup ignores stale bootstrap/status and removes late listeners", async () => {
  const loading = deferred(),
    status = deferred(),
    subscription = deferred();
  const remove = mock.fn();
  commands.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  commands.audio_status.mock.mockImplementationOnce(() => status.promise);
  listen.mock.mockImplementationOnce(async () => () => {});
  listen.mock.mockImplementationOnce(() => subscription.promise, 1);
  const old = connectApplication();
  await setImmediate();
  const oldClose = callback("request-close");
  old.disconnect();
  connection = setConnection(connectApplication());
  await setImmediate();
  loading.resolve({ ...bootstrap, settings: { ...settings, pitch: "12" } });
  subscription.resolve(remove);
  await setImmediate();
  status.resolve({ busy: true, info: { phase: "error" } });
  oldClose();
  await setImmediate();
  assert.equal(settingsStore.getState().settings.pitch, "0");
  assert.equal(audioStore.getState().snapshot.info.phase, "ready");
  assert.equal(remove.mock.callCount(), 1);
  assert.equal(commands.close_application.mock.callCount(), 0);
});
test("failed shutdown allows corrected settings on the next request", async () => {
  connection = setConnection(connectApplication());
  await setImmediate();
  settingsStore.getState().update("pitch", "");
  commands.close_application.mock.mockImplementationOnce(async () => {
    throw new Error("invalid pitch");
  });
  connection.requestClose();
  await setImmediate();
  assert.equal(audioStore.getState().closing, false);
  assert.match(audioStore.getState().error, /invalid pitch/);
  settingsStore.getState().update("pitch", "3");
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.close_application.mock.calls[1].arguments[0].settings.pitch, "3");
});
test("backend switching keeps CPAL endpoints and defers closing until choices finish", async () => {
  connection = setConnection(connectApplication());
  await setImmediate();
  const loading = deferred();
  commands.device_choices.mock.mockImplementation(() => loading.promise);
  const switching = settingsStore.getState().updateBackend("cpal");
  assert.equal(settingsStore.getState().switchingBackend, true);
  assert.equal(settingsStore.getState().settings.device, "");
  assert.equal(settingsStore.getState().settings.outputDevice, "");
  assert.equal(settingsStore.getState().audioAvailable, false);
  connection.requestClose();
  assert.equal(commands.close_application.mock.callCount(), 0);
  loading.resolve({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  });
  await switching;
  assert.equal(settingsStore.getState().resolvedBackend, "cpal");
  assert.deepEqual(settingsStore.getState().outputDevices, ["Headphones"]);
  assert.equal(commands.close_application.mock.callCount(), 1);
  assert.deepEqual(commands.device_choices.mock.calls[0].arguments, [{ backend: "cpal" }]);
});
