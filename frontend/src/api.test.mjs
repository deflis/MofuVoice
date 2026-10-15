import assert from "node:assert/strict";
import { test, mock } from "node:test";
import { handlers, calls, deferred, mainView, settings } from "./testing/storeMocks.mjs";
const { connectWindow, connectLogs } = await import("./api.ts");
const { settingsApi } = await import("./windows/settings/api.ts");

test("connection delivers initial state and updates through the same channel", async () => {
  const received = mock.fn();
  const connection = connectWindow(received, assert.fail);
  await connection.ready;
  assert.equal(received.mock.calls[0].arguments[0].type, "main");
  const channel = calls("connect_window")[0].arguments[1].channel;
  channel.onmessage({ type: "phase", phase: { phase: "running", busy: true, error: null } });
  assert.equal(received.mock.callCount(), 2);
  await connection.close();
  channel.onmessage({ type: "close" });
  assert.equal(received.mock.callCount(), 2);
  await connection.close();
  assert.equal(calls("disconnect_window").length, 1);
});
test("closing before registration completes suppresses delivery and releases its exact connection", async () => {
  const pending = deferred();
  handlers.set("connect_window", () => pending.promise);
  const received = mock.fn();
  const connection = connectWindow(received, assert.fail);
  const args = calls("connect_window")[0].arguments[1];
  const closing = connection.close();
  args.channel.onmessage({ type: "main", view: mainView });
  assert.equal(received.mock.callCount(), 0);
  assert.equal(calls("disconnect_window").length, 0);
  pending.resolve();
  await closing;
  assert.equal(calls("disconnect_window")[0].arguments[1].connectionId, args.connectionId);
});
test("failed registration is visible and does not attempt to disconnect a missing subscription", async () => {
  handlers.set("connect_window", async () => {
    throw new Error("registration failed");
  });
  const errors = mock.fn();
  const connection = connectWindow(assert.fail, errors);
  await assert.rejects(connection.ready, /registration failed/);
  await connection.close();
  assert.equal(errors.mock.callCount(), 1);
  assert.equal(calls("disconnect_window").length, 0);
});

test("a failed disconnect can be retried while message delivery remains stopped", async () => {
  const received = mock.fn();
  const connection = connectWindow(received, assert.fail);
  await connection.ready;
  const channel = calls("connect_window")[0].arguments[1].channel;
  handlers.set("disconnect_window", async () => {
    throw new Error("disconnect failed");
  });
  await assert.rejects(connection.close(), /disconnect failed/);
  channel.onmessage({ type: "close" });
  assert.equal(received.mock.callCount(), 1);
  handlers.delete("disconnect_window");
  await connection.close();
  assert.equal(calls("disconnect_window").length, 2);
});
test("log subscription has its own channel carrying only log payloads", async () => {
  const received = mock.fn();
  const connection = connectLogs(received, assert.fail);
  await connection.ready;
  assert.deepEqual(received.mock.calls[0].arguments[0], { elapsedSeconds: 0, telemetry: null });
  assert.equal(calls("connect_window").length, 0);
  assert.equal(calls("set_logging").length, 0);
  await connection.close();
  assert.equal(calls("disconnect_logs").length, 1);
});
test("settings snapshot, saving and backend discovery retain their command payloads", async () => {
  await settingsApi.getView();
  await settingsApi.save(settings);
  await settingsApi.deviceChoices("cpal");
  await settingsApi.deviceSampleRates({ backend: "cpal", device: "Mic", outputDevice: "Speakers" });
  assert.deepEqual(calls("save_settings")[0].arguments[1], { settings });
  assert.deepEqual(calls("device_sample_rates")[0].arguments[1], {
    backend: "cpal",
    device: "Mic",
    outputDevice: "Speakers",
  });
});
