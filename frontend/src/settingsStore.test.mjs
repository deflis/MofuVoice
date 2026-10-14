import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
const { settings, bootstrap, commands, settingsStore, deferred } =
  await import("./testing/tauriMocks.mjs");

test("settings save preserves its command and current payload", async () => {
  settingsStore.setState(bootstrap);
  assert.equal("snapshot" in settingsStore.getState(), false);
  await settingsStore.getState().save();
  assert.deepEqual(commands.save_settings.mock.calls[0].arguments, [{ settings }]);
  assert.equal(settingsStore.getState().saving, false);
  assert.equal(settingsStore.getState().notice, "設定を保存しました。");
});
test("settings bootstrap ignores results after disconnect", async () => {
  const loading = deferred();
  commands.bootstrap.mock.mockImplementationOnce(() => loading.promise);
  const disconnect = settingsStore.getState().connect();
  disconnect();
  loading.resolve(bootstrap);
  await setImmediate();
  assert.equal(settingsStore.getState().settings, undefined);
});

test("channel selections are saved and sent together with the settings", async () => {
  settingsStore.setState(bootstrap);
  settingsStore.getState().update("inputChannel", 2);
  settingsStore.getState().update("outputChannels", [1, 2]);
  await settingsStore.getState().save();
  const saved = commands.save_settings.mock.calls[0].arguments[0].settings;
  assert.equal(saved.inputChannel, 2);
  assert.deepEqual(saved.outputChannels, [1, 2]);
  settingsStore.getState().update("outputChannels", [2]);
  assert.deepEqual(saved.outputChannels, [1, 2]);
});

test("routing shortcut replaces a custom mapping in one update without changing audio settings", async () => {
  settingsStore.setState({
    ...bootstrap,
    settings: { ...settings, inputChannel: 9, outputChannels: [3, 12] },
  });
  const changes = [];
  const unsubscribe = settingsStore.subscribe((state) => changes.push(state.settings));
  const outputs = [1, 2];
  settingsStore.getState().updateRouting(1, outputs);
  unsubscribe();
  outputs.push(3);
  assert.equal(changes.length, 1);
  assert.deepEqual(changes[0], { ...settings, inputChannel: 1, outputChannels: [1, 2] });
  await settingsStore.getState().save();
  assert.deepEqual(commands.save_settings.mock.calls[0].arguments[0].settings, changes[0]);
});
