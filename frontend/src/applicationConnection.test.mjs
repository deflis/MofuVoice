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

test("saved settings reach the main window and refresh backend choices", async () => {
  connection = setConnection(connectApplication());
  await setImmediate();
  const next = { ...settings, backend: "cpal", pitch: "3" };
  commands.bootstrap.mock.mockImplementation(async () => ({
    ...bootstrap,
    settings: next,
    resolvedBackend: "cpal",
  }));
  callback("settings-saved")({ payload: next });
  await setImmediate();
  assert.equal(settingsStore.getState().settings.pitch, "3");
  assert.equal(settingsStore.getState().resolvedBackend, "cpal");
});

test("closing settings saves the draft and hides without shutting down audio", async () => {
  connection = setConnection(connectApplication(true));
  await setImmediate();
  settingsStore.getState().update("pitch", "4");
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.save_settings.mock.calls[0].arguments[0].settings.pitch, "4");
  assert.equal(commands.close_settings.mock.callCount(), 1);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("settings save failure keeps the window open and allows retry", async () => {
  connection = setConnection(connectApplication(true));
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
  connection = setConnection(connectApplication(true));
  await setImmediate();
  settingsStore.getState().update("pitch", "5");
  callback("request-application-close")({ payload: 1 });
  await setImmediate();
  assert.equal(commands.close_application.mock.calls[0].arguments[0].settings.pitch, "5");
  assert.equal(commands.close_settings.mock.callCount(), 0);
});

test("failed application shutdown does not turn a later settings close into shutdown", async () => {
  connection = setConnection(connectApplication(true));
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
  connection = setConnection(connectApplication(true));
  await setImmediate();
  callback("request-application-close")({ payload: 7 });
  await setImmediate();
  assert.equal(commands.acknowledge_application_close.mock.callCount(), 0);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("settings loading cannot acknowledge application shutdown", async () => {
  const loading = deferred();
  commands.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  connection = setConnection(connectApplication(true));
  callback("request-application-close")({ payload: 8 });
  assert.equal(commands.acknowledge_application_close.mock.callCount(), 0);
  loading.resolve(bootstrap);
  await setImmediate();
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("late acknowledgement cannot start a second shutdown after native fallback", async () => {
  connection = setConnection(connectApplication(true));
  await setImmediate();
  commands.acknowledge_application_close.mock.mockImplementationOnce(async () => false);
  callback("request-application-close")({ payload: 9 });
  await setImmediate();
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("disconnect during acknowledgement hands shutdown back to main", async () => {
  connection = setConnection(connectApplication(true));
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
  connection = setConnection(connectApplication(true));
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
  connection = setConnection(connectApplication(true));
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
