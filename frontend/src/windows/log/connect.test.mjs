import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { commands, connectLog, setConnection, callback } =
  await import("../../testing/tauriMocks.mjs");

let connection;

test("closing the log window hides it without saving or shutting down", async () => {
  connection = setConnection(connectLog());
  await setImmediate();
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.close_log.mock.callCount(), 1);
  assert.equal(commands.save_settings.mock.callCount(), 0);
  assert.equal(commands.close_application.mock.callCount(), 0);
});

test("request-close hides the log window", async () => {
  connection = setConnection(connectLog());
  await setImmediate();
  callback("request-close")();
  await setImmediate();
  assert.equal(commands.close_log.mock.callCount(), 1);
  assert.equal(commands.close_settings.mock.callCount(), 0);
});

test("failed hide can be retried", async () => {
  connection = setConnection(connectLog());
  await setImmediate();
  commands.close_log.mock.mockImplementationOnce(async () => {
    throw new Error("hide failed");
  });
  connection.requestClose();
  await setImmediate();
  connection.requestClose();
  await setImmediate();
  assert.equal(commands.close_log.mock.callCount(), 2);
});
