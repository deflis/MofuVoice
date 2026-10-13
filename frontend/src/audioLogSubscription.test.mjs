import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { mock, test } from "node:test";
const { commands, listen, cleanups, deferred, callback } = await import("./testing/tauriMocks.mjs");
const { subscribeAudioLog } = await import("./audioLogSubscription.ts");

test("closing during delayed listen removes the old listener before reopening", async () => {
  const subscription = deferred();
  const remove = mock.fn();
  const oldLog = mock.fn();
  const newLog = mock.fn();
  listen.mock.mockImplementationOnce(() => subscription.promise);
  const close = subscribeAudioLog(oldLog, assert.fail);
  cleanups.push(close);
  await setImmediate();
  const oldCallback = callback("audio-log");
  close();
  cleanups.push(subscribeAudioLog(newLog, assert.fail));
  subscription.resolve(remove);
  await setImmediate();
  assert.equal(remove.mock.callCount(), 1);
  assert.deepEqual(
    commands.set_logging.mock.calls.map((call) => call.arguments[0].logging),
    [false, true],
  );
  oldCallback({ payload: { elapsedSeconds: 99, telemetry: null } });
  assert.equal(oldLog.mock.callCount(), 0);
  const latest = { elapsedSeconds: 7, telemetry: null };
  listen.mock.calls.at(-1).arguments[1]({ payload: latest });
  assert.equal(newLog.mock.calls.at(-1).arguments[0], latest);
});

test("a pushed log wins over a delayed activation response", async () => {
  const activation = deferred();
  commands.set_logging.mock.mockImplementationOnce(() => activation.promise);
  const onLog = mock.fn();
  cleanups.push(subscribeAudioLog(onLog, assert.fail));
  await setImmediate();
  const pushed = { elapsedSeconds: 10, telemetry: null };
  callback("audio-log")({ payload: pushed });
  activation.resolve({ elapsedSeconds: 0, telemetry: null });
  await setImmediate();
  assert.deepEqual(
    onLog.mock.calls.map((call) => call.arguments[0]),
    [pushed],
  );
});

test("failed listen and activation do not poison subsequent subscriptions", async () => {
  for (const failedCommand of ["listen", "activate"]) {
    const failure = new Error(`${failedCommand} failed`);
    if (failedCommand === "listen") {
      listen.mock.mockImplementationOnce(async () => {
        throw failure;
      });
    } else {
      commands.set_logging.mock.mockImplementationOnce(async () => {
        throw failure;
      });
    }
    const onError = mock.fn();
    const close = subscribeAudioLog(assert.fail, onError);
    cleanups.push(close);
    await setImmediate();
    assert.equal(onError.mock.calls[0].arguments[0], failure);
    close();
    const onLog = mock.fn();
    const closeAgain = subscribeAudioLog(onLog, assert.fail);
    cleanups.push(closeAgain);
    await setImmediate();
    assert.equal(onLog.mock.callCount(), 1);
    closeAgain();
    await setImmediate();
  }
});

test("subscription cleanup removes and deactivates exactly once", async () => {
  const remove = mock.fn();
  listen.mock.mockImplementationOnce(async () => remove);
  const close = subscribeAudioLog(() => {}, assert.fail);
  cleanups.push(close);
  await setImmediate();
  close();
  close();
  await setImmediate();
  assert.equal(remove.mock.callCount(), 1);
  assert.deepEqual(
    commands.set_logging.mock.calls.map((call) => call.arguments[0].logging),
    [true, false],
  );
});
