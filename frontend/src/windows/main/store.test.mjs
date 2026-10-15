import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mainStore,
  main,
  mainView,
  calls,
  handlers,
  push,
  connect,
  tick,
  deferred,
  channels,
} from "../../testing/storeMocks.mjs";
const phase = (name, busy) => ({ type: "phase", phase: { phase: name, busy, error: null } });

test("main state is flat and start/stop transitions follow backend notifications", async () => {
  await connect("main").ready;
  assert.equal(mainStore.getState().status, "idle");
  assert.equal("machine" in mainStore.getState(), false);
  assert.equal("view" in mainStore.getState(), false);
  await main.startAudio(false);
  assert.equal(mainStore.getState().status, "starting");
  assert.deepEqual(calls("start_audio")[0].arguments[1], { inspect: false });
  push(phase("opening", true));
  assert.equal(mainStore.getState().status, "starting");
  push(phase("running", true));
  await main.stopAudio();
  push(phase("stopped", true));
  assert.equal(mainStore.getState().status, "stopping");
  push(phase("stopped", false));
  assert.equal(mainStore.getState().status, "idle");
});
test("invalid actions are ignored and command failures allow retry", async () => {
  await connect("main").ready;
  await main.stopAudio();
  assert.equal(calls("stop_audio").length, 0);
  handlers.set("start_audio", async () => {
    throw new Error("cannot start");
  });
  await main.startAudio(false);
  assert.equal(mainStore.getState().status, "faulted");
  assert.match(mainStore.getState().error, /cannot start/);
  handlers.delete("start_audio");
  await main.startAudio(false);
  assert.equal(mainStore.getState().status, "starting");
});
test("close during startup waits for the worker and runs shutdown once", async () => {
  await connect("main").ready;
  await main.startAudio(false);
  push({ type: "close" });
  assert.equal(calls("close_application").length, 0);
  push(phase("running", true));
  push({ type: "close" });
  await tick();
  assert.equal(calls("close_application").length, 1);
  assert.equal(mainStore.getState().status, "closing");
  push(phase("running", true));
  push({ type: "close" });
  await tick();
  assert.equal(calls("close_application").length, 1);
});
test("failed shutdown keeps the latest received phase and device facts", async () => {
  await connect("main").ready;
  push(phase("running", true));
  const closing = deferred();
  handlers.set("close_application", () => closing.promise);
  push({ type: "close" });
  push(phase("stopped", false));
  push({ type: "main", view: { ...mainView, canStart: false } });
  closing.reject(new Error("save failed"));
  await tick();
  assert.equal(mainStore.getState().status, "idle");
  assert.equal(mainStore.getState().canStart, false);
  assert.match(mainStore.getState().error, /save failed/);
});
test("notifications and old commands cannot modify a replacement connection", async () => {
  await connect("main").ready;
  const old = channels.get("main");
  const starting = deferred();
  handlers.set("start_audio", () => starting.promise);
  const action = main.startAudio(false);
  await connect("main").ready;
  starting.reject(new Error("old command"));
  await action;
  old.onmessage(phase("running", true));
  assert.equal(mainStore.getState().status, "idle");
  assert.equal(mainStore.getState().error, "");
});
test("backend events take priority over late command errors", async () => {
  await connect("main").ready;
  const starting = deferred();
  handlers.set("start_audio", () => starting.promise);
  const action = main.startAudio(false);
  push(phase("running", true));
  starting.reject(new Error("late error"));
  await action;
  assert.equal(mainStore.getState().status, "running");
  assert.equal(mainStore.getState().error, "");
});
test("connection failures expose retry and action failures preserve audio state", async () => {
  handlers.set("connect_window", async () => {
    throw new Error("connection failed");
  });
  await assert.rejects(connect("main").ready);
  assert.equal(mainStore.getState().status, "bootstrapFailed");
  handlers.delete("connect_window");
  await connect("main").ready;
  push(phase("running", true));
  handlers.set("open_log", async () => {
    throw new Error("cannot open log");
  });
  await main.openLog();
  assert.equal(mainStore.getState().status, "running");
  assert.match(mainStore.getState().error, /cannot open log/);
});

test("a start failure after opening still faults the matching command", async () => {
  await connect("main").ready;
  const starting = deferred();
  handlers.set("start_audio", () => starting.promise);
  const action = main.startAudio(false);
  push(phase("opening", true));
  starting.reject(new Error("driver open failed"));
  await action;
  assert.equal(mainStore.getState().status, "faulted");
  assert.match(mainStore.getState().error, /driver open failed/);
});
test("unknown phases fault visibly and a recovered phase clears the audio error", async () => {
  await connect("main").ready;
  push(phase("unknown", false));
  assert.equal(mainStore.getState().status, "faulted");
  assert.match(mainStore.getState().error, /unknown/);
  push(phase("running", true));
  assert.equal(mainStore.getState().status, "running");
  assert.equal(mainStore.getState().error, "");
});
