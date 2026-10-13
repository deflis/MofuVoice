import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { commands, settingsStore, audioStore, logStore, cleanups, deferred, callback } =
  await import("./testing/tauriMocks.mjs");

test("log viewers share a subscription and isolate updates from other stores", async () => {
  let settingsUpdates = 0,
    audioUpdates = 0;
  const removeSettings = settingsStore.subscribe(() => settingsUpdates++);
  const removeAudio = audioStore.subscribe(() => audioUpdates++);
  cleanups.push(removeSettings, removeAudio);
  const close1 = logStore.getState().connect(),
    close2 = logStore.getState().connect();
  cleanups.push(close1, close2);
  await setImmediate();
  const oldCallback = callback("audio-log");
  oldCallback({ payload: { elapsedSeconds: 4, telemetry: null } });
  assert.equal(logStore.getState().log.elapsedSeconds, 4);
  assert.equal(settingsUpdates, 0);
  assert.equal(audioUpdates, 0);
  close1();
  close1();
  assert.equal(commands.set_logging.mock.callCount(), 1);
  close2();
  await setImmediate();
  assert.equal(logStore.getState().log, null);
  assert.deepEqual(
    commands.set_logging.mock.calls.map((call) => call.arguments[0].logging),
    [true, false],
  );
  oldCallback({ payload: { elapsedSeconds: 99, telemetry: null } });
  assert.equal(logStore.getState().log, null);
  removeSettings();
  removeAudio();
});
test("rapid log reopen finishes old shutdown before new activation", async () => {
  const activation = deferred();
  commands.set_logging.mock.mockImplementationOnce(() => activation.promise);
  const close = logStore.getState().connect();
  cleanups.push(close);
  await setImmediate();
  close();
  const closeAgain = logStore.getState().connect();
  cleanups.push(closeAgain);
  activation.resolve({ elapsedSeconds: 0, telemetry: null });
  await setImmediate();
  assert.deepEqual(
    commands.set_logging.mock.calls.map((call) => call.arguments[0].logging),
    [true, false, true],
  );
});
