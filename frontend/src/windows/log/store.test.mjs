import assert from "node:assert/strict";
import { test } from "node:test";
import {
  logStore,
  log,
  handlers,
  calls,
  push,
  connect,
  tick,
  deferred,
  channels,
  info,
} from "../../testing/storeMocks.mjs";

test("window status populates info but logs arrive only on the dedicated channel", async () => {
  await connect("log").ready;
  assert.deepEqual(logStore.getState().info, info);
  assert.equal(logStore.getState().log, null);
  await log.setLogging(true);
  assert.equal(calls("connect_logs").length, 1);
  assert.equal(calls("set_logging").length, 0);
  const payload = { elapsedSeconds: 5, telemetry: null };
  push(payload, "logs");
  assert.equal(logStore.getState().log, payload);
  push({ type: "info", info: { ...info, phase: "running" } }, "log");
  assert.equal(logStore.getState().log, payload);
});
test("turning off unsubscribes, clears logs and ignores late payloads", async () => {
  await connect("log").ready;
  await log.setLogging(true);
  const old = channels.get("logs");
  const stopping = log.setLogging(false);
  old.onmessage({ elapsedSeconds: 99, telemetry: null });
  assert.equal(logStore.getState().log, null);
  await stopping;
  assert.equal(calls("disconnect_logs").length, 1);
  assert.equal(logStore.getState().logging, false);
});
test("rapid resubscription closes the prior stream before registering a new one", async () => {
  await connect("log").ready;
  const pending = deferred();
  handlers.set("connect_logs", () => pending.promise);
  const first = log.setLogging(true);
  await tick();
  const second = log.setLogging(false);
  const third = log.setLogging(true);
  handlers.delete("connect_logs");
  pending.resolve();
  await Promise.all([first, second, third]);
  assert.equal(calls("connect_logs").length, 2);
  assert.equal(calls("disconnect_logs").length, 1);
  const sequence = (await import("../../testing/storeMocks.mjs")).invoke.mock.calls
    .map((call) => call.arguments[0])
    .filter((command) => command.includes("logs"));
  assert.deepEqual(sequence, ["connect_logs", "disconnect_logs", "connect_logs"]);
});
test("subscription failure is visible and a fresh subscription can retry", async () => {
  await connect("log").ready;
  handlers.set("connect_logs", async () => {
    throw new Error("cannot subscribe");
  });
  await log.setLogging(true);
  assert.equal(logStore.getState().logging, false);
  assert.match(logStore.getState().error, /cannot subscribe/);
  handlers.delete("connect_logs");
  await log.setLogging(true);
  assert.equal(logStore.getState().logging, true);
});
test("native close stops the log stream before hiding the window", async () => {
  await connect("log").ready;
  await log.setLogging(true);
  push({ type: "close" }, "log");
  await tick();
  assert.equal(calls("disconnect_logs").length, 1);
  assert.equal(calls("close_log").length, 1);
  assert.equal(logStore.getState().log, null);
});
test("clear resets the transient cache without issuing system operations", () => {
  logStore.getState().setLog({ elapsedSeconds: 2, telemetry: null });
  logStore.getState().clear();
  assert.equal(logStore.getState().log, null);
  assert.equal(calls("connect_logs").length, 0);
  assert.equal(calls("disconnect_logs").length, 0);
});
