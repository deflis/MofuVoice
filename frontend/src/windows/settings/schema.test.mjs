import assert from "node:assert/strict";
import { test } from "node:test";
import * as v from "valibot";
import {
  settings,
  settingsView,
  settingsStore,
  calls,
  handlers,
} from "../../testing/storeMocks.mjs";
const { settingsSchema } = await import("./schema.ts");
const { settingsApi } = await import("./api.ts");

test("TD-PSOLA selection retains independent formant shift and validates IPC", async () => {
  settingsStore.getState().voice.setFormantShift(2.53);
  settingsStore.getState().voice.setPreserveFormants(false);
  settingsStore.getState().voice.setPitchAlgorithm("td-psola");
  assert.equal(settingsStore.getState().voice.formantShift, 2.53);
  assert.equal(settingsStore.getState().voice.preserveFormants, true);
  await settingsStore.getState().save();
  assert.equal(calls("save_settings")[0].arguments[1].settings.voice.pitchAlgorithm, "td-psola");
  assert.equal(
    v.safeParse(settingsSchema, {
      ...settings,
      voice: { ...settings.voice, pitchAlgorithm: "td-psola", formantShift: 2.53 },
    }).success,
    true,
  );
  for (const voice of [
    { ...settings.voice, pitchAlgorithm: "td-psola", preserveFormants: false },
  ]) {
    assert.equal(v.safeParse(settingsSchema, { ...settings, voice }).success, false);
  }
});

test("pitch algorithm selection is validated and saved", async () => {
  settingsStore.getState().voice.setPitchAlgorithm("pvsola");
  await settingsStore.getState().save();
  assert.equal(calls("save_settings")[0].arguments[1].settings.voice.pitchAlgorithm, "pvsola");
  for (const pitchAlgorithm of ["phase-locked", "pvsola", "pvsola-sync", "td-psola"]) {
    assert.equal(
      v.safeParse(settingsSchema, { ...settings, voice: { ...settings.voice, pitchAlgorithm } })
        .success,
      true,
    );
  }
  for (const pitchAlgorithm of ["psola", "", null, undefined]) {
    assert.equal(
      v.safeParse(settingsSchema, { ...settings, voice: { ...settings.voice, pitchAlgorithm } })
        .success,
      false,
    );
  }
});

test("saving extracts only numeric settings from slices and strips every action", async () => {
  const voiceBefore = settingsStore.getState().voice;
  settingsStore.getState().devices.setFrames(128);
  settingsStore.getState().voice.setPitch(3.25);
  settingsStore.getState().eq.setBandQ(1, 1.2);
  await settingsStore.getState().save();
  const payload = calls("save_settings")[0].arguments[1].settings;
  assert.equal(payload.devices.frames, 128);
  assert.equal(payload.voice.pitch, 3.25);
  assert.equal(payload.eq.bands[1].q, 1.2);
  assert.equal("save" in payload, false);
  assert.equal("setPitch" in payload.voice, false);
  assert.equal("setFrames" in payload.devices, false);
  assert.equal("reset" in payload.eq, false);
  assert.equal(voiceBefore.pitch, settings.voice.pitch);
});

test("empty number inputs, nonfinite values and fractional integers cannot reach IPC", async () => {
  for (const value of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
    settingsStore.getState().voice.setPitch(value);
    await assert.rejects(settingsStore.getState().save());
  }
  settingsStore.getState().voice.setPitch(0);
  for (const value of [0, 64.5, Number.NaN, 8193]) {
    settingsStore.getState().devices.setFrames(value);
    await assert.rejects(settingsStore.getState().save());
  }
  assert.equal(calls("save_settings").length, 0);
});

test("cross-field constraints and numeric types are checked without coercion", () => {
  for (const voice of [
    { ...settings.voice, pitch: "3" },
    { ...settings.voice, preserveFormants: false, formantShift: 2 },
  ]) {
    assert.equal(v.safeParse(settingsSchema, { ...settings, voice }).success, false);
  }
  assert.equal(
    v.safeParse(settingsSchema, { ...settings, level: { ...settings.level, floorDb: -10 } })
      .success,
    false,
  );
  assert.equal(
    v.safeParse(settingsSchema, {
      ...settings,
      devices: { ...settings.devices, outputChannels: [1, 1] },
    }).success,
    false,
  );
  assert.equal(v.safeParse(settingsSchema, settings).success, true);
});

test("invalid backend snapshots fail visibly instead of populating the settings store", async () => {
  handlers.set("settings_view", async () => ({
    ...settingsView,
    settings: { ...settings, voice: { ...settings.voice, pitch: "0" } },
  }));
  await assert.rejects(settingsApi.getView());
  assert.equal(settingsStore.getState().voice.pitch, settings.voice.pitch);
});

test("automatic gate mode ignores invalid manual threshold after mode switch", async () => {
  settingsStore.getState().noise.setThresholdMode("manual");
  settingsStore.getState().noise.setThresholdDb(Number.NaN);
  await assert.rejects(settingsStore.getState().save());
  settingsStore.getState().noise.setThresholdMode("auto");
  assert.equal(settingsStore.getState().noise.thresholdDb, -50);
  await settingsStore.getState().save();
  assert.equal(calls("save_settings").at(-1).arguments[1].settings.noise.thresholdMode, "auto");
});

test("spectral windows validate, save, and clamp synthesis when analysis shrinks", async () => {
  const voice = settingsStore.getState().voice;
  voice.setAnalysisWindow(8192);
  voice.setSynthesisWindow(2048);
  await settingsStore.getState().save();
  const saved = calls("save_settings")[0].arguments[1].settings.voice;
  assert.equal(saved.analysisWindow, 8192);
  assert.equal(saved.synthesisWindow, 2048);
  settingsStore.getState().voice.setAnalysisWindow(1024);
  assert.equal(settingsStore.getState().voice.synthesisWindow, 1024);
  for (const changes of [
    { analysisWindow: 1536 },
    { synthesisWindow: 128 },
    { analysisWindow: 1024, synthesisWindow: 2048 },
  ]) {
    assert.equal(
      v.safeParse(settingsSchema, { ...settings, voice: { ...settings.voice, ...changes } })
        .success,
      false,
    );
  }
});
