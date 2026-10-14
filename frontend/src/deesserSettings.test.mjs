import assert from "node:assert/strict";
import { test } from "node:test";
const { bootstrap, settingsStore, commands } = await import("./testing/tauriMocks.mjs");

test("de-esser mode changes retain custom controls and save the selected mode", async () => {
  settingsStore.setState(bootstrap);
  const original = settingsStore.getState().settings;
  const parameters = { ...original.deesserParameters, frequencyHz: "6500", rangeDb: "9" };
  settingsStore.getState().update("deesserParameters", parameters);
  for (const mode of ["pro", "simple", "pro"]) {
    settingsStore.getState().update("deesserMode", mode);
    assert.deepEqual(settingsStore.getState().settings.deesserParameters, parameters);
  }
  settingsStore.getState().update("deesser", true);
  settingsStore.getState().update("deesser", false);
  assert.deepEqual(settingsStore.getState().settings.deesserParameters, parameters);
  assert.deepEqual(settingsStore.getState().settings.level, original.level);
  assert.equal(settingsStore.getState().settings.pitch, original.pitch);
  await settingsStore.getState().save();
  const saved = commands.save_settings.mock.calls.at(-1).arguments[0].settings;
  assert.equal(saved.deesserMode, "pro");
  assert.deepEqual(saved.deesserParameters, parameters);
});
