import assert from "node:assert/strict";
import { mock, test } from "node:test";
import { registerTypeScript } from "../../testing/registerTypeScript.mjs";
registerTypeScript();
const { queryDeviceSampleRates } = await import("./deviceSampleRates.ts");

const query = { backend: "cpal", device: "Mic", outputDevice: "Headphones" };
const rates = { input: [44100, 48000], output: [48000], common: [48000] };

test("queries both selected devices and delivers their rates", async () => {
  const getRates = mock.fn(async () => rates);
  const receive = mock.fn();
  queryDeviceSampleRates(query, receive, getRates);
  await Promise.resolve();
  assert.deepEqual(getRates.mock.calls[0].arguments[0], query);
  assert.deepEqual(receive.mock.calls[0].arguments[0], { rates });
});

test("device changes discard delayed results from the previous selection", async () => {
  const old = Promise.withResolvers();
  const getRates = mock.fn(({ device }) =>
    device === "Mic" ? old.promise : Promise.resolve(rates),
  );
  const receive = mock.fn();
  const cancel = queryDeviceSampleRates(query, receive, getRates);
  cancel();
  queryDeviceSampleRates({ ...query, device: "New mic" }, receive, getRates);
  await Promise.resolve();
  old.resolve({ input: [96000], output: [96000], common: [96000] });
  await Promise.resolve();
  assert.equal(receive.mock.calls.length, 1);
  assert.deepEqual(receive.mock.calls[0].arguments[0], { rates });
});

test("query failure is reported without claiming no supported rates", async () => {
  const getRates = mock.fn(async () => {
    throw new Error("Device disconnected");
  });
  const receive = mock.fn();
  queryDeviceSampleRates(query, receive, getRates);
  await Promise.resolve();
  assert.deepEqual(receive.mock.calls[0].arguments[0], { error: "Error: Device disconnected" });
});

test("closing settings ignores a delayed failure", async () => {
  const pending = Promise.withResolvers();
  const getRates = mock.fn(() => pending.promise);
  const receive = mock.fn();
  queryDeviceSampleRates(query, receive, getRates)();
  pending.reject(new Error("Device disconnected"));
  await Promise.resolve();
  assert.equal(receive.mock.calls.length, 0);
});
