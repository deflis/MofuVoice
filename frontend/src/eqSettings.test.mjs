import assert from "node:assert/strict";
import { test } from "node:test";
import { settingsStore, settings, calls } from "./testing/storeMocks.mjs";
test("EQ mode changes preserve custom bands and do not modify voice or level settings", async () => {
  settingsStore.getState().eq.setBandFrequency(1, 2200);
  settingsStore.getState().eq.setBandGainDb(1, 3.5);
  for (const mode of ["pro", "simple"]) {
    settingsStore.getState().eq.setMode(mode);
  }
  await settingsStore.getState().save();
  const saved = calls("save_settings")[0].arguments[1].settings;
  assert.equal(saved.eq.bands[1].frequency, 2200);
  assert.equal(saved.eq.bands[1].gainDb, 3.5);
  assert.deepEqual(saved.eq.bands[0], settings.eq.bands[0]);
  assert.deepEqual(saved.level, settings.level);
  assert.equal(saved.voice.pitch, settings.voice.pitch);
});

test("variable EQ band counts survive editing, saving and neutral reset", async () => {
  for (const count of [1, 4, 8]) {
    const bands = Array.from({ length: count }, (_, index) => ({
      frequency: 500 + index * 500,
      gainDb: 2,
      q: 1,
    }));
    settingsStore.getState().eq.setBands(bands);
    settingsStore.getState().eq.setBandFrequency(count - 1, 4500);
    settingsStore.getState().eq.setBandGainDb(count - 1, 3);
    settingsStore.getState().eq.setBandQ(count - 1, 1.5);
    assert.equal(bands[count - 1].frequency, 500 + (count - 1) * 500);
    await settingsStore.getState().save();
    const saved = calls("save_settings").at(-1).arguments[1].settings;
    assert.equal(saved.eq.bands.length, count);
    assert.deepEqual(saved.eq.bands[count - 1], { frequency: 4500, gainDb: 3, q: 1.5 });
    settingsStore.getState().eq.reset();
    assert.equal(settingsStore.getState().eq.bands.length, count);
    assert.equal(settingsStore.getState().eq.bands[count - 1].frequency, 4500);
    assert.equal(settingsStore.getState().eq.bands[count - 1].q, 1.5);
    assert.ok(settingsStore.getState().eq.bands.every((band) => band.gainDb === 0));
  }
  settingsStore.getState().eq.setBands([]);
  await assert.rejects(settingsStore.getState().save());
});
