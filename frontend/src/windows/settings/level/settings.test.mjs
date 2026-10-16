import assert from "node:assert/strict";
import { test } from "node:test";
import { settingsStore, calls } from "../../../testing/storeMocks.mjs";

test("level and compressor switches save independently without changing parameters", async () => {
  settingsStore.getState().level.setTargetDb(-18);
  settingsStore.getState().level.setRatio(5);
  for (const enabled of [false, true]) {
    for (const compressorEnabled of [false, true]) {
      settingsStore.getState().level.setEnabled(enabled);
      settingsStore.getState().level.setCompressorEnabled(compressorEnabled);
      await settingsStore.getState().save();
      const saved = calls("save_settings").at(-1).arguments[1].settings.level;
      assert.equal(saved.enabled, enabled);
      assert.equal(saved.compressorEnabled, compressorEnabled);
      assert.equal(saved.targetDb, -18);
      assert.equal(saved.ratio, 5);
    }
  }
});
