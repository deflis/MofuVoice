import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
import { logApi, logStore, cleanups, deferred, callback } from "../../testing/storeMocks.mjs";

test("log_view does not overwrite a newer log-info payload", async () => {
  const loading = deferred();
  logApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(logStore.getState().mount());
  const newer = {
    phase: "running",
    error: null,
    device: "Mic",
    outputDevice: "Headphones",
    backend: "cpal",
    sampleRate: 48000,
    inputs: 1,
    outputs: 2,
    frames: 128,
    periodMs: 2.67,
    dspMs: 0,
    inputLatencyMs: 1,
    outputLatencyMs: 2,
  };
  callback("log-info")({ payload: newer });
  assert.deepEqual(logStore.getState().info, newer);
  loading.resolve({
    ...newer,
    phase: "ready",
    device: null,
  });
  await setImmediate();
  assert.deepEqual(logStore.getState().info, newer);
});

test("rapid logging reopen disables the previous activation before starting again", async () => {
  cleanups.push(logStore.getState().mount());
  await setImmediate();
  const activating = deferred();
  logApi.setLogging.mock.mockImplementationOnce(() => activating.promise);
  logStore.getState().setLogging(true);
  await setImmediate();
  logStore.getState().setLogging(false);
  logStore.getState().setLogging(true);
  activating.resolve({ elapsedSeconds: 99, telemetry: null });
  await setImmediate();
  assert.deepEqual(
    logApi.setLogging.mock.calls.map((call) => call.arguments[0]),
    [true, false, true],
  );
  assert.equal(logStore.getState().log.elapsedSeconds, 0);
});

test("activation failure removes its listener and permits a fresh retry", async () => {
  cleanups.push(logStore.getState().mount());
  await setImmediate();
  let removed = 0;
  logApi.onLog = async () => () => {
    removed += 1;
  };
  logApi.setLogging.mock.mockImplementationOnce(async () => {
    throw new Error("activation failed");
  });
  logStore.getState().setLogging(true);
  await setImmediate();
  assert.equal(logStore.getState().logging, false);
  assert.equal(removed, 1);
  assert.match(logStore.getState().error, /activation failed/);
  logStore.getState().setLogging(true);
  await setImmediate();
  assert.equal(logStore.getState().logging, true);
  assert.equal(logStore.getState().error, "");
});

test("late log listeners are removed once and cannot publish after unmount", async () => {
  const disconnect = logStore.getState().mount();
  const subscription = deferred();
  let receive;
  let removed = 0;
  logApi.onLog = (onLog) => {
    receive = onLog;
    return subscription.promise;
  };
  logStore.getState().setLogging(true);
  await setImmediate();
  disconnect();
  disconnect();
  subscription.resolve(() => {
    removed += 1;
  });
  await setImmediate();
  receive({ elapsedSeconds: 99, telemetry: null });
  assert.equal(logStore.getState().log, null);
  assert.equal(logStore.getState().logging, false);
  assert.equal(removed, 1);
  assert.deepEqual(
    logApi.setLogging.mock.calls.map((call) => call.arguments[0]),
    [false],
  );
});

test("pushed logs win over a delayed activation snapshot", async () => {
  cleanups.push(logStore.getState().mount());
  const activating = deferred();
  logApi.setLogging.mock.mockImplementationOnce(() => activating.promise);
  logStore.getState().setLogging(true);
  await setImmediate();
  const pushed = { elapsedSeconds: 8, telemetry: null };
  callback("audio-log")({ payload: pushed });
  activating.resolve({ elapsedSeconds: 0, telemetry: null });
  await setImmediate();
  assert.equal(logStore.getState().log, pushed);
});

test("an old hide failure cannot overwrite the remounted log window", async () => {
  const disconnect = logStore.getState().mount();
  const hiding = deferred();
  logApi.hide.mock.mockImplementationOnce(() => hiding.promise);
  logStore.getState().requestClose();
  disconnect();
  cleanups.push(logStore.getState().mount());
  await setImmediate();
  hiding.reject(new Error("old hide failed"));
  await setImmediate();
  assert.equal(logStore.getState().error, "");
});
