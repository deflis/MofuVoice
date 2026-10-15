import assert from "node:assert/strict";
import { test } from "node:test";
import { settingsStore, calls } from "./testing/storeMocks.mjs";
test("de-esser modes and enable changes retain custom controls in the saved payload", async () => {
  settingsStore.getState().deesser.setFrequencyHz(6500);
  settingsStore.getState().deesser.setRangeDb(9);
  for (const mode of ["pro", "simple", "pro"]) {
    settingsStore.getState().deesser.setMode(mode);
  }
  settingsStore.getState().deesser.setEnabled(true);
  settingsStore.getState().deesser.setEnabled(false);
  await settingsStore.getState().save();
  const saved = calls("save_settings")[0].arguments[1].settings;
  assert.equal(saved.deesser.mode, "pro");
  assert.equal(saved.deesser.frequencyHz, 6500);
  assert.equal(saved.deesser.rangeDb, 9);
});
