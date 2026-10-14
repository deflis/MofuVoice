import assert from "node:assert/strict";
import { test } from "node:test";
const { bootstrap, settingsStore, commands } = await import("./testing/tauriMocks.mjs");

test("level unit changes preserve targets and persist through save", async () => {
  settingsStore.setState(bootstrap);
  const original = settingsStore.getState().settings.level;
  for (const unit of ["dbfs", "lufs"]) {
    settingsStore.getState().updateLevel("unit", unit);
    assert.deepEqual(settingsStore.getState().settings.level, { ...original, unit });
    await settingsStore.getState().save();
    assert.equal(commands.save_settings.mock.calls.at(-1).arguments[0].settings.level.unit, unit);
  }
});

test("reset restores default level unit while preserving the enabled state", () => {
  settingsStore.setState(bootstrap);
  settingsStore.getState().updateLevel("enabled", true);
  settingsStore.getState().updateLevel("unit", "dbfs");
  settingsStore.getState().updateLevel("targetDb", "-20");
  settingsStore.getState().resetLevel();
  assert.deepEqual(settingsStore.getState().settings.level, {
    ...bootstrap.levelDefaults,
    enabled: true,
  });
});
