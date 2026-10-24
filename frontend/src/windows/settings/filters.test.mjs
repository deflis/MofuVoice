import assert from "node:assert/strict";
import { test } from "node:test";
import {
  settingsStore,
  settingsView,
  settingsListener,
  calls,
  connect,
  tick,
  handlers,
  deferred,
} from "../../testing/storeMocks.mjs";

test("three EQ stages keep independent bands through bypass, save, and reload", async () => {
  await connect("settings").ready;
  await tick();
  const state = settingsStore.getState();
  state.inputEq.setBandGainDb(0, 4);
  state.preConversionEq.setBandGainDb(0, -3);
  state.eq.setBandGainDb(0, 2);
  state.inputEq.setEnabled(true);
  settingsStore.getState().inputEq.setEnabled(false);
  await settingsStore.getState().save();
  const saved = calls("save_settings").at(-1).arguments[1].settings;
  assert.equal(saved.inputEq.enabled, false);
  assert.equal(saved.inputEq.bands[0].gainDb, 4);
  assert.equal(saved.preConversionEq.bands[0].gainDb, -3);
  assert.equal(saved.eq.bands[0].gainDb, 2);
  assert.notEqual(state.inputEq.bands, state.preConversionEq.bands);
  assert.equal("setBandGainDb" in saved.inputEq, false);
  handlers.set("settings_view", async () => ({ ...settingsView, settings: saved }));
  await connect("settings").ready;
  await tick();
  assert.equal(settingsStore.getState().inputEq.bands[0].gainDb, 4);
  assert.equal(settingsStore.getState().preConversionEq.bands[0].gainDb, -3);
  assert.equal(settingsStore.getState().eq.bands[0].gainDb, 2);
});

test("disabled filters retain parameters and still reject invalid settings", async () => {
  settingsStore.getState().voice.setPitch(5);
  settingsStore.getState().voice.setEnabled(false);
  settingsStore.getState().compressor.setThresholdDb(-22);
  settingsStore.getState().compressor.setEnabled(false);
  await settingsStore.getState().save();
  const saved = calls("save_settings").at(-1).arguments[1].settings;
  assert.equal(saved.voice.pitch, 5);
  assert.equal(saved.voice.enabled, false);
  assert.equal(saved.compressor.thresholdDb, -22);
  for (const [key, setter, invalid, valid] of [
    ["limiter", "setCeilingDb", 1, -1],
    ["dereverb", "setAmount", Number.NaN, 0.5],
    ["reverb", "setDecayMs", 0, 1200],
    ["compressor", "setRatio", 0, 3],
  ]) {
    settingsStore.getState()[key][setter](invalid);
    await assert.rejects(settingsStore.getState().save());
    settingsStore.getState()[key][setter](valid);
  }
});

test("dirty state follows edits, revert, failed save and only the saved snapshot", async () => {
  await connect("settings").ready;
  await tick();
  const connection = settingsListener.useSettingsConnection;
  assert.equal(connection.getState().dirty, false);
  settingsStore.getState().reverb.setMix(0.3);
  assert.equal(connection.getState().dirty, true);
  settingsStore.getState().reverb.setMix(0.15);
  assert.equal(connection.getState().dirty, false);
  settingsStore.getState().reverb.setMix(0.4);
  handlers.set("save_settings", async () => {
    throw new Error("disk failure");
  });
  await assert.rejects(settingsStore.getState().save());
  assert.equal(connection.getState().dirty, true);
  const saving = deferred();
  handlers.set("save_settings", () => saving.promise);
  const pending = settingsStore.getState().save();
  settingsStore.getState().reverb.setMix(0.5);
  saving.resolve();
  await pending;
  assert.equal(connection.getState().dirty, true);
  handlers.delete("save_settings");
  await settingsStore.getState().save();
  assert.equal(connection.getState().dirty, false);
});
