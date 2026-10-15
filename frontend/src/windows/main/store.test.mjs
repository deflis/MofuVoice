import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { mainView, commands, listen, mainStore, cleanups, deferred, callback } =
  await import("../../testing/tauriMocks.mjs");

test("start and stop use server settings and drive the state machine", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "idle");
  await mainStore.getState().start(false);
  assert.deepEqual(commands.start_audio.mock.calls[0].arguments, [{ inspect: false }]);
  assert.equal(mainStore.getState().machine.status, "starting");
  callback("audio-phase")({ payload: { phase: "running", error: null } });
  assert.equal(mainStore.getState().machine.status, "running");
  await mainStore.getState().stop();
  assert.equal(mainStore.getState().machine.status, "stopping");
  callback("audio-phase")({ payload: { phase: "ready", error: null } });
  assert.equal(mainStore.getState().machine.status, "idle");
});

test("main-view updates replace setup facts without holding settings", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  const next = {
    ...mainView,
    canStart: false,
    setup: { ...mainView.setup, rememberedDeviceMissing: true },
  };
  callback("main-view")({ payload: next });
  assert.deepEqual(mainStore.getState().machine.view, next);
});

test("close waits while starting then shuts down once running", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  await mainStore.getState().start(false);
  mainStore.getState().requestClose();
  assert.equal(commands.close_application.mock.callCount(), 0);
  callback("audio-phase")({ payload: { phase: "running", error: null } });
  assert.equal(commands.close_application.mock.callCount(), 1);
  assert.equal(mainStore.getState().machine.status, "closing");
});

test("bootstrap failure exposes retry without closing", async () => {
  const loading = deferred();
  commands.main_view.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(mainStore.getState().mount());
  mainStore.getState().requestClose();
  loading.reject(new Error("main view failed"));
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "bootstrapFailed");
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("open_log failure keeps running audio and reports action error", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  await mainStore.getState().start(false);
  callback("audio-phase")({ payload: { phase: "running", error: null } });
  commands.open_log.mock.mockImplementationOnce(async () => {
    throw new Error("log window failed");
  });
  await mainStore.getState().openLog();
  assert.equal(mainStore.getState().machine.status, "running");
  assert.match(mainStore.getState().error, /log window failed/);
});

test("faulted audio can be retried when canStart is true", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  commands.start_audio.mock.mockImplementationOnce(async () => {
    throw new Error("device missing");
  });
  await mainStore.getState().start(false);
  assert.equal(mainStore.getState().machine.status, "faulted");
  commands.start_audio.mock.mockImplementation(async () => {});
  await mainStore.getState().start(false);
  assert.equal(mainStore.getState().machine.status, "starting");
});

test("unmount ignores late main_view and removes listeners", async () => {
  const loading = deferred();
  const remove = { phase: 0, view: 0, close: 0 };
  commands.main_view.mock.mockImplementationOnce(() => loading.promise);
  listen.mock.mockImplementation(async (event) => {
    if (event === "audio-phase") {
      return () => {
        remove.phase += 1;
      };
    }
    if (event === "main-view") {
      return () => {
        remove.view += 1;
      };
    }
    return () => {
      remove.close += 1;
    };
  });
  const disconnect = mainStore.getState().mount();
  disconnect();
  loading.resolve(mainView);
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "bootstrapping");
  assert.deepEqual(remove, { phase: 1, view: 1, close: 1 });
});
