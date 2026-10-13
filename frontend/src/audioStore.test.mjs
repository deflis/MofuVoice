import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { mock, test } from "node:test";
const { settings, commands, listen, audioStore, cleanups, deferred, callback } =
  await import("./testing/tauriMocks.mjs");

test("audio commands preserve command names and settings payloads", async () => {
  await audioStore.getState().start(settings, false);
  await audioStore.getState().start(settings, true);
  assert.deepEqual(
    commands.start_audio.mock.calls.map((call) => call.arguments),
    [[{ settings, inspect: false }], [{ settings, inspect: true }]],
  );
  await audioStore.getState().stop();
  await audioStore.getState().spectrum();
  assert.deepEqual(commands.stop_audio.mock.calls[0].arguments, [undefined]);
  assert.deepEqual(commands.open_spectrum.mock.calls[0].arguments, [undefined]);
});
test("status is pushed without polling or subscribing to logs", async () => {
  cleanups.push(audioStore.getState().connect(() => {}));
  await setImmediate();
  assert.equal(
    listen.mock.calls.some((call) => call.arguments[0] === "audio-log"),
    false,
  );
  const update = { busy: true, info: { phase: "running", backend: "cpal" } };
  callback("audio-status")({ payload: update });
  assert.equal(audioStore.getState().snapshot, update);
  assert.equal("elapsedSeconds" in audioStore.getState().snapshot.info, false);
  assert.equal(commands.set_logging.mock.callCount(), 0);
});
test("disconnect before status subscription resolves skips initial status fetch", async () => {
  const subscription = deferred();
  const remove = mock.fn();
  listen.mock.mockImplementationOnce(() => subscription.promise);
  const disconnect = audioStore.getState().connect(() => {});
  disconnect();
  subscription.resolve(remove);
  await setImmediate();
  assert.equal(remove.mock.callCount(), 1);
  assert.equal(commands.audio_status.mock.callCount(), 0);
});

test("initial and command status responses cannot overwrite newer pushed status", async () => {
  for (const initial of [true, false]) {
    const status = deferred();
    commands.audio_status.mock.mockImplementationOnce(() => status.promise);
    const disconnect = audioStore.getState().connect(() => {});
    cleanups.push(disconnect);
    await setImmediate();
    if (!initial) {
      status.resolve({ busy: false, info: { phase: "ready" } });
      await setImmediate();
      const commandStatus = deferred();
      commands.audio_status.mock.mockImplementationOnce(() => commandStatus.promise);
      const operation = audioStore.getState().start(settings, false);
      await setImmediate();
      const pushed = { busy: true, info: { phase: "running" } };
      listen.mock.calls.at(-2).arguments[1]({ payload: pushed });
      commandStatus.resolve({ busy: false, info: { phase: "opening" } });
      await operation;
      assert.equal(audioStore.getState().snapshot, pushed);
    } else {
      const pushed = { busy: true, info: { phase: "running" } };
      callback("audio-status")({ payload: pushed });
      status.resolve({ busy: false, info: { phase: "ready" } });
      await setImmediate();
      assert.equal(audioStore.getState().snapshot, pushed);
    }
    disconnect();
  }
});
