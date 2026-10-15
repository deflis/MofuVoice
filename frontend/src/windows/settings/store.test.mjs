import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
import {
  settings,
  settingsView,
  settingsApi,
  settingsStore,
  cleanups,
  deferred,
  callback,
} from "../../testing/storeMocks.mjs";

test("settings mount loads settings_view only", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  assert.equal(settingsApi.getView.mock.callCount(), 1);
  assert.equal(settingsStore.getState().status, "ready");
  assert.deepEqual(settingsStore.getState().settings, settings);
});

test("close saves draft then hides settings", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsStore.getState().update("pitch", "3");
  settingsStore.getState().requestClose();
  await setImmediate();
  assert.equal(settingsApi.save.mock.callCount(), 1);
  assert.equal(settingsApi.save.mock.calls[0].arguments[0].pitch, "3");
  assert.equal(settingsApi.hide.mock.callCount(), 1);
});

test("close queued during automatic save does not hide after reopen", async () => {
  const saving = deferred();
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsApi.save.mock.mockImplementationOnce(() => saving.promise);
  settingsStore.getState().requestClose();
  assert.equal(settingsStore.getState().status, "saving");
  settingsStore.getState().requestClose();
  saving.resolve(undefined);
  await setImmediate();
  assert.equal(settingsApi.hide.mock.callCount(), 1);
  assert.equal(settingsApi.save.mock.callCount(), 1);
  settingsStore.getState().update("pitch", "4");
  await settingsStore.getState().save();
  assert.equal(settingsApi.save.mock.callCount(), 2);
  assert.equal(settingsApi.hide.mock.callCount(), 1);
  assert.equal(settingsStore.getState().status, "ready");
});

test("load failure allows hide without save", async () => {
  settingsApi.getView.mock.mockImplementationOnce(async () => {
    throw new Error("settings failed");
  });
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsStore.getState().requestClose();
  await setImmediate();
  assert.equal(settingsApi.save.mock.callCount(), 0);
  assert.equal(settingsApi.hide.mock.callCount(), 1);
});

test("close queued during failed load hides the window", async () => {
  const loading = deferred();
  settingsApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(settingsStore.getState().mount());
  settingsStore.getState().requestClose();
  loading.reject(new Error("settings failed"));
  await setImmediate();
  assert.equal(settingsApi.save.mock.callCount(), 0);
  assert.equal(settingsApi.hide.mock.callCount(), 1);
});

test("close queued during backend switch resumes after device_choices", async () => {
  const switching = deferred();
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsApi.deviceChoices.mock.mockImplementationOnce(() => switching.promise);
  const update = settingsStore.getState().updateBackend("cpal");
  callback("request-application-close")({ payload: 9 });
  await setImmediate();
  assert.equal(settingsApi.closeApplication.mock.callCount(), 0);
  switching.resolve({
    devices: ["Mic"],
    outputDevices: ["Headphones"],
    audioAvailable: true,
    deviceError: "",
    resolvedBackend: "cpal",
  });
  await update;
  await setImmediate();
  assert.equal(settingsApi.closeApplication.mock.callCount(), 1);
});

test("settings_view does not overwrite a newer audio-phase busy flag", async () => {
  const loading = deferred();
  settingsApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(settingsStore.getState().mount());
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  assert.equal(settingsStore.getState().audioBusy, true);
  loading.resolve({ ...settingsView, audioBusy: false });
  await setImmediate();
  assert.equal(settingsStore.getState().audioBusy, true);
});

test("application close acknowledgment delegates shutdown after save", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  callback("request-application-close")({ payload: 7 });
  await setImmediate();
  assert.equal(settingsApi.acknowledgeClose.mock.callCount(), 1);
  assert.equal(settingsApi.closeApplication.mock.callCount(), 1);
});

test("audio-phase updates audioBusy lock", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  assert.equal(settingsStore.getState().audioBusy, true);
  callback("audio-phase")({ payload: { phase: "ready", error: null, busy: false } });
  assert.equal(settingsStore.getState().audioBusy, false);
});

test("backend switch uses device_choices without bootstrap", async () => {
  settingsStore.setState({
    ...settingsStore.getState(),
    ...settingsView,
    status: "ready",
  });
  await settingsStore.getState().updateBackend("cpal");
  assert.equal(settingsApi.deviceChoices.mock.callCount(), 1);
  assert.equal(settingsStore.getState().resolvedBackend, "cpal");
});

test("close during loading waits for settings_view", async () => {
  const loading = deferred();
  settingsApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(settingsStore.getState().mount());
  settingsStore.getState().requestClose();
  assert.equal(settingsApi.hide.mock.callCount(), 0);
  loading.resolve(settingsView);
  await setImmediate();
  assert.equal(settingsApi.save.mock.callCount(), 1);
  assert.equal(settingsApi.hide.mock.callCount(), 1);
});

test("settings mutations and commands obey the same lock as the controls", async () => {
  for (const [status, audioBusy] of [
    ["loading", false],
    ["loadFailed", false],
    ["saving", false],
    ["switchingBackend", false],
    ["closing", false],
    ["ready", true],
  ]) {
    settingsStore.setState({ ...settingsView, status, audioBusy });
    const before = settingsStore.getState();
    before.update("pitch", "12");
    before.update("backend", "cpal");
    before.updateRouting(2, [1, 2]);
    before.updateLevel("targetDb", "-20");
    before.updateEqBand(0, "gainDb", "3");
    before.resetLevel();
    before.resetEq();
    await before.save();
    await before.updateBackend("cpal");
    assert.equal(settingsStore.getState(), before);
  }
  assert.equal(settingsApi.save.mock.callCount(), 0);
  assert.equal(settingsApi.deviceChoices.mock.callCount(), 0);
});

test("save and device responses cannot modify settings after remount", async () => {
  for (const action of ["save", "updateBackend"]) {
    for (const fail of [false, true]) {
      const disconnect = settingsStore.getState().mount();
      await setImmediate();
      const old = deferred();
      const apiAction = action === "save" ? "save" : "deviceChoices";
      settingsApi[apiAction].mock.mockImplementationOnce(() => old.promise);
      const operation = settingsStore.getState()[action]("cpal");
      disconnect();
      cleanups.push(settingsStore.getState().mount());
      await setImmediate();
      const current = settingsStore.getState();
      if (fail) {
        old.reject(new Error("old response failed"));
      } else {
        old.resolve({ devices: ["old"], resolvedBackend: "old" });
      }
      await operation;
      assert.equal(settingsStore.getState(), current);
    }
  }
});

test("automatic saves from an old mount cannot hide or shut down a reopened window", async () => {
  for (const delegated of [false, true]) {
    for (const fail of [false, true]) {
      const disconnect = settingsStore.getState().mount();
      await setImmediate();
      const saving = deferred();
      settingsApi.save.mock.mockImplementationOnce(() => saving.promise);
      if (delegated) {
        callback("request-application-close")({ payload: 13 });
      } else {
        settingsStore.getState().requestClose();
      }
      await setImmediate();
      disconnect();
      cleanups.push(settingsStore.getState().mount());
      await setImmediate();
      const current = settingsStore.getState();
      if (fail) {
        saving.reject(new Error("old close save failed"));
      } else {
        saving.resolve();
      }
      await setImmediate();
      assert.equal(settingsStore.getState(), current);
      assert.equal(settingsApi.hide.mock.callCount(), 0);
      assert.equal(settingsApi.closeApplication.mock.callCount(), 0);
    }
  }
  assert.equal(settingsApi.fallbackClose.mock.callCount(), 2);
});

test("generic backend update refreshes choices instead of retaining old devices", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  settingsStore.getState().update("backend", "cpal");
  assert.equal(settingsStore.getState().status, "switchingBackend");
  assert.equal(settingsStore.getState().settings.device, "");
  await setImmediate();
  assert.equal(settingsApi.deviceChoices.mock.calls[0].arguments[0], "cpal");
  assert.equal(settingsStore.getState().resolvedBackend, "cpal");
});

test("save failure permits correction and retry without automatically retrying queued closes", async () => {
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  const saving = deferred();
  settingsApi.save.mock.mockImplementationOnce(() => saving.promise);
  settingsStore.getState().requestClose();
  settingsStore.getState().requestClose();
  saving.reject(new Error("save failed"));
  await setImmediate();
  assert.equal(settingsStore.getState().status, "ready");
  assert.match(settingsStore.getState().error, /save failed/);
  assert.equal(settingsApi.save.mock.callCount(), 1);
  assert.equal(settingsApi.hide.mock.callCount(), 0);
  settingsStore.getState().update("pitch", "4");
  settingsStore.getState().requestClose();
  await setImmediate();
  assert.equal(settingsApi.hide.mock.callCount(), 1);
  assert.equal(settingsApi.save.mock.calls[1].arguments[0].pitch, "4");
});

test("close acknowledgements are exclusive and late acceptance returns to native fallback", async () => {
  const disconnect = settingsStore.getState().mount();
  await setImmediate();
  const acknowledgment = deferred();
  settingsApi.acknowledgeClose.mock.mockImplementationOnce(() => acknowledgment.promise);
  const onClose = callback("request-application-close");
  onClose({ payload: 17 });
  onClose({ payload: 17 });
  assert.equal(settingsApi.acknowledgeClose.mock.callCount(), 1);
  disconnect();
  cleanups.push(settingsStore.getState().mount());
  await setImmediate();
  acknowledgment.resolve(true);
  await setImmediate();
  assert.deepEqual(settingsApi.fallbackClose.mock.calls[0].arguments, [17]);
  assert.equal(settingsApi.closeApplication.mock.callCount(), 0);
});
