import assert from "node:assert/strict";
import { test } from "node:test";
import { registerTypeScript } from "../../../testing/registerTypeScript.mjs";
registerTypeScript();
const { channelLimits } = await import("./deviceChannels.ts");

test("channel choices preserve compatible duplex combinations", () => {
  const layouts = [
    { sampleRate: 48000, input: 8, output: 2 },
    { sampleRate: 48000, input: 2, output: 8 },
    { sampleRate: 44100, input: 16, output: 16 },
  ];
  assert.deepEqual(channelLimits(layouts, 48000, 8, [1, 2]), {
    available: true,
    input: 8,
    output: 2,
  });
  assert.deepEqual(channelLimits(layouts, 48000, 2, [1, 8]), {
    available: true,
    input: 2,
    output: 8,
  });
  assert.deepEqual(channelLimits(layouts, 48000, 8, [8]), { available: true, input: 2, output: 2 });
  assert.deepEqual(channelLimits(layouts, 48000, 8, []), { available: true, input: 8, output: 2 });
  assert.deepEqual(channelLimits([], 48000, 1, [1]), { available: false, input: 0, output: 0 });
});
