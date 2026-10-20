import test from "node:test";
import assert from "node:assert/strict";
import { percentToSemitones, semitonesToPercent } from "./pitchUnits.ts";

test("pitch percentages map neutral and octave endpoints exactly", () => {
  for (const [percent, semitones] of [
    [50, -12],
    [100, 0],
    [200, 12],
  ]) {
    assert.equal(percentToSemitones(percent), semitones);
    assert.equal(semitonesToPercent(semitones), percent);
  }
});

test("fractional pitch survives unit switching without quantization", () => {
  for (const semitones of [-11.99, -3.25, 0.01, 5.37, 11.99]) {
    assert.ok(Math.abs(percentToSemitones(semitonesToPercent(semitones)) - semitones) < 1e-12);
  }
  assert.ok(Math.abs(semitonesToPercent(percentToSemitones(125)) - 125) < 1e-12);
});

test("empty and nonpositive percentages remain invalid", () => {
  for (const percent of [NaN, Infinity, -Infinity, 0, -50]) {
    assert.ok(Number.isNaN(percentToSemitones(percent)));
  }
});
