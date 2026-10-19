import test from "node:test";
import assert from "node:assert/strict";
import { pitchLatency } from "./latency.ts";

test("pitch latency includes PSOLA tracking, asymmetric support and optional formant stage", () => {
  for (const algorithm of ["td-psola", "td-psola-resampled", "fd-psola"]) {
    assert.equal(pitchLatency(algorithm, 512, 2048, 512, 0), 5376);
    assert.equal(
      pitchLatency(algorithm, 512, 2048, 512, 3),
      algorithm === "td-psola-resampled" ? 5376 : 5888,
    );
    assert.equal(pitchLatency(algorithm, 1024, 1024, 1024, 0), 4352);
    assert.equal(pitchLatency(algorithm, 256, 8192, 256, 0), 9088);
  }
  assert.equal(pitchLatency("pvsola", 2048, 8192, 2048, 4), 4608);
  assert.equal(pitchLatency("phase-locked", 256, 8192, 256, 4), 256);
  assert.equal(pitchLatency("pvsola-sync", 1024, 1024, 1024, 0), 1024);
});

test("mixed PSOLA and vocoder stages keep their own window sizes", () => {
  assert.equal(pitchLatency("td-psola", 2048, 1024, 256, 4), 4864 + 2048);
  assert.equal(pitchLatency("fd-psola", 256, 8192, 2048, 4), 8448 + 256);
  assert.equal(pitchLatency("td-psola-resampled", 2048, 1024, 256, 4), 4864);
  assert.equal(pitchLatency("phase-locked", 256, 8192, 2048, 4), 256);
});
