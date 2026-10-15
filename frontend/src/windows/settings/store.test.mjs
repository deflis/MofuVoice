import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { settings, settingsView, commands, settingsStore, cleanups, deferred, callback } =
  await import("../../testing/tauriMocks.mjs");

test("settings mount loads settings_view only", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  assert.equal(commands.settings_view.mock.callCount(), 1);
  assert.equal(settingsStore.getState().status, "ready");
  assert.deepEqual(settingsStore.getState().settings, settings);
});

test("close saves draft then hides settings", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsStore.getState().update("pitch", "3");
  settingsStore.getState().requestClose();
  await setImmediate();
  assert.equal(commands.save_settings.mock.callCount(), 1);
  assert.equal(commands.save_settings.mock.calls[0].arguments[0].settings.pitch, "3");
  assert.equal(commands.close_settings.mock.callCount(), 1);
});

test("load failure allows hide without save", async () => {
  commands.settings_view.mock.mockImplementationOnce(async () => {
    throw new Error("settings failed");
  });
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsStore.getState().requestClose();
  await setImmediate();
  assert.equal(commands.save_settings.mock.callCount(), 0);
  assert.equal(commands.close_settings.mock.callCount(), 1);
});

test("application close acknowledgment delegates shutdown after save", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  callback("request-application-close")({ payload: 7 });
  await setImmediate();
  assert.equal(commands.acknowledge_application_close.mock.callCount(), 1);
  assert.equal(commands.close_application.mock.callCount(), 1);
});

test("audio-phase updates audioBusy lock", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  callback("audio-phase")({ payload: { phase: "running", error: null } });
  assert.equal(settingsStore.getState().audioBusy, true);
  callback("audio-phase")({ payload: { phase: "ready", error: null } });
  assert.equal(settingsStore.getState().audioBusy, false);
});

test("backend switch uses device_choices without bootstrap", async () => {
  settingsStore.setState({
    ...settingsStore.getState(),
    ...settingsView,
    status: "ready",
  });
  await settingsStore.getState().updateBackend("cpal");
  assert.equal(commands.device_choices.mock.callCount(), 1);
  assert.equal(settingsStore.getState().resolvedBackend, "cpal");
});

test("close during loading waits for settings_view", async () => {
  const loading = deferred();
  commands.settings_view.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(settingsStore.getState().mount());
  settingsStore.getState().requestClose();
  assert.equal(commands.close_settings.mock.callCount(), 0);
  loading.resolve(settingsView);
  await setImmediate();
  assert.equal(commands.save_settings.mock.callCount(), 1);
  assert.equal(commands.close_settings.mock.callCount(), 1);
});
