import assert from "node:assert/strict";
import { test } from "node:test";
import { eqResponseDb, rumbleResponseDb } from "./eqResponse.ts";
import { compressorOutputDb } from "./transfer.ts";

test("EQ previews include DSP positive-gain headroom, trim, and retained disabled settings", () => {
  for (const rate of [44100, 48000]) {
    for (const gainDb of [-12, -3, 0, 6, 12]) {
      const settings = {
        enabled: false,
        outputDb: -3,
        bands: [{ frequency: 1000, gainDb, q: 0.8 }],
      };
      const expected = -3 + gainDb - Math.max(0, gainDb);
      assert.ok(Math.abs(eqResponseDb(settings, rate, 1000) - expected) < 1e-9);
      assert.equal(
        eqResponseDb(settings, rate, 1000),
        eqResponseDb({ ...settings, enabled: true }, rate, 1000),
      );
    }
  }
});

// Independently evaluate the DSP difference equation and measure a sine after settling.
function measuredResponse(settings, rate, frequency) {
  const filters = settings.bands.map((band) => {
    const a = 10 ** (band.gainDb / 40),
      w = (2 * Math.PI * band.frequency) / rate;
    const alpha = Math.sin(w) / (2 * band.q),
      a0 = 1 + alpha / a;
    return {
      b: [(1 + alpha * a) / a0, (-2 * Math.cos(w)) / a0, (1 - alpha * a) / a0],
      a: [(-2 * Math.cos(w)) / a0, (1 - alpha / a) / a0],
      x1: 0,
      x2: 0,
      y1: 0,
      y2: 0,
    };
  });
  const scale =
    10 **
    ((settings.outputDb - settings.bands.reduce((sum, b) => sum + Math.max(0, b.gainDb), 0)) / 20);
  let power = 0;
  for (let i = 0; i < rate * 2; i++) {
    let sample = Math.sin((2 * Math.PI * frequency * i) / rate) * scale;
    for (const filter of filters) {
      const out =
        filter.b[0] * sample +
        filter.b[1] * filter.x1 +
        filter.b[2] * filter.x2 -
        filter.a[0] * filter.y1 -
        filter.a[1] * filter.y2;
      filter.x2 = filter.x1;
      filter.x1 = sample;
      filter.y2 = filter.y1;
      filter.y1 = out;
      sample = out;
    }
    if (i >= rate) {
      power += sample * sample;
    }
  }
  return 10 * Math.log10(power / rate / 0.5);
}
test("EQ frequency response matches sample-domain biquads at supported rates", () => {
  const settings = {
    outputDb: -4,
    bands: [
      { frequency: 250, gainDb: 6, q: 0.7 },
      { frequency: 3000, gainDb: -9, q: 2 },
      { frequency: 8000, gainDb: 4, q: 0.8 },
    ],
  };
  for (const rate of [44100, 48000]) {
    for (const frequency of [80, 250, 1000, 3000, 8000, 15000]) {
      assert.ok(
        Math.abs(
          eqResponseDb(settings, rate, frequency) - measuredResponse(settings, rate, frequency),
        ) < 1e-7,
        `${rate} Hz sample rate, ${frequency} Hz tone`,
      );
    }
  }
});

test("invalid EQ and unsupported rates withhold the response instead of clamping", () => {
  const settings = { outputDb: 0, bands: [{ frequency: 1000, gainDb: 0, q: 1 }] };
  for (const changed of [
    { ...settings, outputDb: NaN },
    { ...settings, outputDb: 1 },
    { ...settings, bands: [] },
    { ...settings, bands: [{ frequency: 1000, gainDb: 13, q: 1 }] },
    { ...settings, bands: [{ frequency: NaN, gainDb: 0, q: 1 }] },
    { ...settings, bands: [{ frequency: 1000, gainDb: 0, q: 0 }] },
  ]) {
    assert.ok(Number.isNaN(eqResponseDb(changed, 48000, 1000)));
  }
  assert.ok(Number.isNaN(eqResponseDb(settings, 32000, 1000)));
  assert.ok(Number.isNaN(eqResponseDb(settings, 48000, 25000)));
});

test("input HPF has the DSP Butterworth corner and sample-rate-dependent warping", () => {
  for (const rate of [44100, 48000]) {
    assert.ok(Math.abs(rumbleResponseDb(rate, 80) + 10 * Math.log10(2)) < 1e-12);
    assert.ok(rumbleResponseDb(rate, 20) < -24);
    assert.ok(rumbleResponseDb(rate, 1000) > -0.001);
  }
  assert.notEqual(rumbleResponseDb(44100, 200), rumbleResponseDb(48000, 200));
  assert.ok(Number.isNaN(rumbleResponseDb(32000, 80)));
});

test("POST compressor preview matches 6 dB soft knee without inventing a hard corner", () => {
  assert.equal(compressorOutputDb(-20, -12, 4), -20);
  assert.equal(compressorOutputDb(-15, -12, 4), -15);
  assert.equal(compressorOutputDb(-12, -12, 4), -12.5625);
  assert.equal(compressorOutputDb(-9, -12, 4), -11.25);
  assert.equal(compressorOutputDb(0, -12, 4), -9);
  for (const input of [-60, -15, -12, -9, 0]) {
    assert.equal(compressorOutputDb(input, -12, 1), input);
  }
});
