import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { bootstrap, commands, settingsStore, connectSettings, setConnection, deferred, callback } =
  await import("../../testing/tauriMocks.mjs");

let connection;

test("closing settings saves the draft and hides without shutting down audio", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  settingsStore.getState().update("pitch", "4");
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.save_settings.mock.calls[0].arguments[0].settings.pitch, "4");
  assert.equal(commands.close_settings.mock.callCount(), 1);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("settings save failure keeps the window open and allows retry", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  commands.save_settings.mock.mockImplementationOnce(async () => {
    throw new Error("invalid settings");
  });
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.close_settings.mock.callCount(), 0);
  assert.match(settingsStore.getState().error, /invalid settings/);
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.close_settings.mock.callCount(), 1);
});

test("application shutdown uses the settings window's unsaved draft", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  settingsStore.getState().update("pitch", "5");
  callback("request-application-close")({ payload: 1 });
  await setImmediate();
  assert.equal(commands.close_application.mock.calls[0].arguments[0].settings.pitch, "5");
  assert.equal(commands.close_settings.mock.callCount(), 0);
});

test("failed application shutdown does not turn a later settings close into shutdown", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  commands.close_application.mock.mockImplementationOnce(async () => {
    throw new Error("save failed");
  });
  callback("request-application-close")({ payload: 1 });
  await setImmediate();
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.close_application.mock.callCount(), 1);
  assert.equal(commands.close_settings.mock.callCount(), 1);
});

test("settings bootstrap failure leaves shutdown to the main-window fallback", async () => {
  commands.bootstrap.mock.mockImplementationOnce(async () => {
    throw new Error("bootstrap failed");
  });
  connection = setConnection(connectSettings());
  await setImmediate();
  callback("request-application-close")({ payload: 7 });
  await setImmediate();
  assert.equal(commands.acknowledge_application_close.mock.callCount(), 0);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("settings loading cannot acknowledge application shutdown", async () => {
  const loading = deferred();
  commands.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  connection = setConnection(connectSettings());
  callback("request-application-close")({ payload: 8 });
  assert.equal(commands.acknowledge_application_close.mock.callCount(), 0);
  loading.resolve(bootstrap);
  await setImmediate();
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("late acknowledgement cannot start a second shutdown after native fallback", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  commands.acknowledge_application_close.mock.mockImplementationOnce(async () => false);
  callback("request-application-close")({ payload: 9 });
  await setImmediate();
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("disconnect during acknowledgement hands shutdown back to main", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  const acknowledgement = deferred();
  commands.acknowledge_application_close.mock.mockImplementationOnce(() => acknowledgement.promise);
  callback("request-application-close")({ payload: 10 });
  connection.disconnect();
  acknowledgement.resolve(true);
  await setImmediate();
  assert.deepEqual(commands.fallback_application_close.mock.calls[0].arguments, [
    { requestId: 10 },
  ]);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("acknowledged settings shutdown waits for a pending device switch and uses its draft", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  const switching = deferred();
  commands.device_choices.mock.mockImplementationOnce(() => switching.promise);
  const operation = settingsStore.getState().updateBackend("cpal");
  callback("request-application-close")({ payload: 11 });
  await setImmediate();
  assert.equal(commands.acknowledge_application_close.mock.callCount(), 1);
  assert.equal(commands.close_application.mock.callCount(), 0);
  switching.resolve({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  });
  await operation;
  assert.equal(commands.close_application.mock.calls[0].arguments[0].settings.backend, "cpal");
});

test("disconnect after acknowledgement returns a queued shutdown to main", async () => {
  connection = setConnection(connectSettings());
  await setImmediate();
  const switching = deferred();
  commands.device_choices.mock.mockImplementationOnce(() => switching.promise);
  const operation = settingsStore.getState().updateBackend("cpal");
  callback("request-application-close")({ payload: 12 });
  await setImmediate();
  connection.disconnect();
  assert.deepEqual(commands.fallback_application_close.mock.calls[0].arguments, [
    { requestId: 12 },
  ]);
  switching.resolve({
    devices: [],
    outputDevices: [],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  });
  await operation;
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("settings bootstrap failure still allows the settings window to close without saving", async () => {
  commands.bootstrap.mock.mockImplementationOnce(async () => {
    throw new Error("bootstrap failed");
  });
  connection = setConnection(connectSettings());
  await setImmediate();
  callback("request-close")();
  await setImmediate();
  assert.equal(settingsStore.getState().settings, undefined);
  assert.equal(commands.close_settings.mock.callCount(), 1);
  assert.equal(commands.save_settings.mock.callCount(), 0);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("failed hide after settings bootstrap failure can be retried", async () => {
  commands.bootstrap.mock.mockImplementationOnce(async () => {
    throw new Error("bootstrap failed");
  });
  connection = setConnection(connectSettings());
  await setImmediate();
  commands.close_settings.mock.mockImplementationOnce(async () => {
    throw new Error("hide failed");
  });
  callback("request-close")();
  await setImmediate();
  assert.match(settingsStore.getState().error, /hide failed/);
  callback("request-close")();
  await setImmediate();
  assert.equal(commands.close_settings.mock.callCount(), 2);
  assert.equal(commands.save_settings.mock.callCount(), 0);
  assert.equal(commands.close_application.mock.callCount(), 0);
});
