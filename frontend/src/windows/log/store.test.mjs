import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { commands, logStore, cleanups, deferred, callback } =
  await import("../../testing/tauriMocks.mjs");

test("log_view does not overwrite a newer log-info payload", async () => {
  const loading = deferred();
  commands.log_view.mock.mockImplementationOnce(() => loading.promise);
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
