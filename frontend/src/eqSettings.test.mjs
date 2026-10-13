import assert from "node:assert/strict";
import { test } from "node:test";
const { bootstrap, settingsStore, commands } = await import("./testing/tauriMocks.mjs");

test("EQ mode changes preserve bands and persist through save", async () => {
  settingsStore.setState(bootstrap);
  settingsStore.getState().updateEqBand(1, "frequency", "2200");
  settingsStore.getState().updateEqBand(1, "gainDb", "3.5");
  const original = settingsStore.getState().settings.eq;
  for (const mode of ["pro", "simple"]) {
    settingsStore.getState().update("eq", { ...settingsStore.getState().settings.eq, mode });
    assert.deepEqual(settingsStore.getState().settings.eq.bands, original.bands);
  }
  await settingsStore.getState().save();
  assert.equal(
    commands.save_settings.mock.calls.at(-1).arguments[0].settings.eq.bands[1].frequency,
    "2200",
  );
});

test("EQ changes and reset leave voice and level settings alone", () => {
  settingsStore.setState(bootstrap);
  const original = settingsStore.getState().settings;
  settingsStore
    .getState()
    .update("eq", { ...original.eq, enabled: true, mode: "pro", outputDb: "-2" });
  settingsStore.getState().updateEqBand(0, "q", "2");
  settingsStore.getState().updateEqBand(0, "gainDb", "-4");
  assert.deepEqual(settingsStore.getState().settings.eq.bands[1], original.eq.bands[1]);
  settingsStore.getState().resetEq();
  const result = settingsStore.getState().settings;
  assert.deepEqual(result.eq, { ...original.eq, enabled: true, mode: "pro" });
  assert.deepEqual(result.level, original.level);
  assert.equal(result.pitch, original.pitch);
});
