import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
import {
  mainView,
  mainApi,
  listen,
  mainStore,
  cleanups,
  deferred,
  callback,
} from "../../testing/storeMocks.mjs";

test("start and stop use server settings and drive the state machine", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "idle");
  await mainStore.getState().start(false);
  assert.deepEqual(mainApi.start.mock.calls[0].arguments, [false]);
  assert.equal(mainStore.getState().machine.status, "starting");
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  assert.equal(mainStore.getState().machine.status, "running");
  await mainStore.getState().stop();
  assert.equal(mainStore.getState().machine.status, "stopping");
  callback("audio-phase")({ payload: { phase: "ready", error: null, busy: false } });
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
  assert.equal(mainApi.close.mock.callCount(), 0);
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  assert.equal(mainApi.close.mock.callCount(), 1);
  assert.equal(mainStore.getState().machine.status, "closing");
});

test("bootstrap failure exposes retry without closing", async () => {
  const loading = deferred();
  mainApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(mainStore.getState().mount());
  mainStore.getState().requestClose();
  loading.reject(new Error("main view failed"));
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "bootstrapFailed");
  assert.equal(mainApi.close.mock.callCount(), 0);
});

test("open_log failure keeps running audio and reports action error", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  await mainStore.getState().start(false);
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  mainApi.openLog.mock.mockImplementationOnce(async () => {
    throw new Error("log window failed");
  });
  await mainStore.getState().openLog();
  assert.equal(mainStore.getState().machine.status, "running");
  assert.match(mainStore.getState().error, /log window failed/);
});

test("faulted audio can be retried when canStart is true", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  mainApi.start.mock.mockImplementationOnce(async () => {
    throw new Error("device missing");
  });
  await mainStore.getState().start(false);
  assert.equal(mainStore.getState().machine.status, "faulted");
  mainApi.start.mock.mockImplementation(async () => {});
  await mainStore.getState().start(false);
  assert.equal(mainStore.getState().machine.status, "starting");
});

test("stopped phase keeps stopping until worker busy clears", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  await mainStore.getState().start(false);
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  await mainStore.getState().stop();
  callback("audio-phase")({ payload: { phase: "stopped", error: null, busy: true } });
  assert.equal(mainStore.getState().machine.status, "stopping");
  callback("audio-phase")({ payload: { phase: "stopped", error: null, busy: false } });
  assert.equal(mainStore.getState().machine.status, "idle");
});

test("main_view phase snapshot restores running after remount", async () => {
  mainApi.getView.mock.mockImplementationOnce(async () => ({
    ...mainView,
    phase: { phase: "running", error: null, busy: true },
  }));
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "running");
});

test("audio-phase during bootstrap wins over stale main_view phase", async () => {
  const loading = deferred();
  mainApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(mainStore.getState().mount());
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  loading.resolve({
    ...mainView,
    phase: { phase: "ready", error: null, busy: false },
  });
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "running");
});

test("close_application failure restores running so Stop stays available", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  await mainStore.getState().start(false);
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  mainApi.close.mock.mockImplementationOnce(async () => {
    throw new Error("settings save failed");
  });
  mainStore.getState().requestClose();
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "running");
  assert.match(mainStore.getState().error, /settings save failed/);
});

test("unmount ignores late main_view and removes listeners", async () => {
  const loading = deferred();
  const remove = { phase: 0, view: 0, close: 0 };
  mainApi.getView.mock.mockImplementationOnce(() => loading.promise);
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

test("main-view received during bootstrap wins over the older snapshot", async () => {
  const loading = deferred();
  mainApi.getView.mock.mockImplementationOnce(() => loading.promise);
  cleanups.push(mainStore.getState().mount());
  const next = { ...mainView, canStart: false };
  callback("main-view")({ payload: next });
  loading.resolve(mainView);
  await setImmediate();
  assert.equal(mainStore.getState().machine.view, next);
});

test("late start and stop failures cannot fault a remounted main window", async () => {
  for (const action of ["start", "stop"]) {
    const disconnect = mainStore.getState().mount();
    await setImmediate();
    if (action === "stop") {
      callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
    }
    const old = deferred();
    mainApi[action].mock.mockImplementationOnce(() => old.promise);
    const operation = mainStore.getState()[action](false);
    disconnect();
    cleanups.push(mainStore.getState().mount());
    await setImmediate();
    const current = mainStore.getState();
    old.reject(new Error("old command failed"));
    await operation;
    assert.equal(mainStore.getState(), current);
  }
});

test("new phase events take precedence over a late start command failure", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  const starting = deferred();
  mainApi.start.mock.mockImplementationOnce(() => starting.promise);
  const operation = mainStore.getState().start(false);
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  starting.reject(new Error("late start response"));
  await operation;
  assert.equal(mainStore.getState().machine.status, "running");
  assert.equal(mainStore.getState().error, "");
});

test("failed close restores the phase received while closing", async () => {
  cleanups.push(mainStore.getState().mount());
  await setImmediate();
  callback("audio-phase")({ payload: { phase: "running", error: null, busy: true } });
  const closing = deferred();
  mainApi.close.mock.mockImplementationOnce(() => closing.promise);
  mainStore.getState().requestClose();
  callback("audio-phase")({ payload: { phase: "stopped", error: null, busy: false } });
  closing.reject(new Error("close failed"));
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "idle");
  assert.match(mainStore.getState().error, /close failed/);
});

test("old cleanup and close failures cannot affect a remounted main window", async () => {
  const disconnect = mainStore.getState().mount();
  await setImmediate();
  const closing = deferred();
  mainApi.close.mock.mockImplementationOnce(() => closing.promise);
  mainStore.getState().requestClose();
  disconnect();
  const nextLoad = deferred();
  mainApi.getView.mock.mockImplementationOnce(() => nextLoad.promise);
  cleanups.push(mainStore.getState().mount());
  disconnect();
  nextLoad.resolve(mainView);
  await setImmediate();
  closing.reject(new Error("old close failed"));
  await setImmediate();
  assert.equal(mainStore.getState().machine.status, "idle");
  assert.equal(mainStore.getState().error, "");
});
