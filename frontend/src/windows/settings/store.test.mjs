import assert from "node:assert/strict";
import { test } from "node:test";
import {
  settingsStore,
  settingsListener,
  settings,
  settingsView,
  toSettings,
  calls,
  handlers,
  push,
  connect,
  tick,
  deferred,
  channels,
} from "../../testing/storeMocks.mjs";
const { useSettingsConnection } = settingsListener;

test("settings load one snapshot into feature slices and preserve new audio busy events", async () => {
  const loading = deferred();
  handlers.set("settings_view", () => loading.promise);
  await connect("settings").ready;
  push({ type: "phase", phase: { phase: "running", busy: true, error: null } });
  loading.resolve(settingsView);
  await tick();
  assert.equal(calls("settings_view").length, 1);
  assert.equal(settingsStore.getState().voice.pitch, settings.voice.pitch);
  assert.equal("settings" in settingsStore.getState(), false);
  assert.equal(useSettingsConnection.getState().audioBusy, true);
  push({ type: "phase", phase: { phase: "stopped", busy: false, error: null } });
  assert.equal(useSettingsConnection.getState().audioBusy, false);
});
test("numeric slice values round-trip the complete backend payload and saving excludes actions", async () => {
  assert.deepEqual(toSettings(settingsStore.getState()), settings);
  settingsStore.getState().voice.setPitch(4);
  settingsStore.getState().level.setRatio(6);
  settingsStore.getState().deesser.setFrequencyHz(6500);
  settingsStore.getState().eq.setBandGainDb(1, 3);
  await settingsStore.getState().save();
  const saved = calls("save_settings")[0].arguments[1].settings;
  assert.equal(saved.voice.pitch, 4);
  assert.equal(saved.level.ratio, 6);
  assert.equal(saved.deesser.frequencyHz, 6500);
  assert.equal(saved.eq.bands[1].gainDb, 3);
  assert.equal(
    Object.values(saved).some((value) => typeof value === "function"),
    false,
  );
  assert.equal(saved.devices.sampleRate, settings.devices.sampleRate);
});
test("settings edits are not overwritten by subsequent status notifications", async () => {
  await connect("settings").ready;
  await tick();
  settingsStore.getState().voice.setPitch(5);
  for (const busy of [true, false]) {
    push({ type: "phase", phase: { phase: busy ? "running" : "stopped", busy, error: null } });
  }
  assert.equal(settingsStore.getState().voice.pitch, 5);
  assert.equal(calls("settings_view").length, 1);
});
test("close saves draft before hiding and duplicate requests share a save", async () => {
  await connect("settings").ready;
  await tick();
  const saving = deferred();
  handlers.set("save_settings", () => saving.promise);
  settingsStore.getState().voice.setPitch(3);
  push({ type: "close" });
  push({ type: "close" });
  assert.equal(calls("save_settings").length, 1);
  assert.equal(calls("close_settings").length, 0);
  saving.resolve();
  await tick();
  assert.equal(calls("close_settings").length, 1);
  assert.equal(calls("save_settings")[0].arguments[1].settings.voice.pitch, 3);
});
test("application close saves before shutdown without acknowledge/fallback handshakes", async () => {
  await connect("settings").ready;
  await tick();
  push({ type: "applicationClose" });
  push({ type: "applicationClose" });
  await tick();
  assert.equal(calls("save_settings").length, 1);
  assert.equal(calls("close_application").length, 1);
  assert.equal(calls("close_settings").length, 0);
  assert.equal(calls("acknowledge_application_close").length, 0);
});
test("close during backend discovery waits, preserves sample rate, then saves and shuts down", async () => {
  await connect("settings").ready;
  await tick();
  settingsStore.getState().devices.setSampleRate(44100);
  const discovery = deferred();
  handlers.set("device_choices", () => discovery.promise);
  const switching = settingsListener.changeBackend("cpal");
  assert.equal(settingsStore.getState().devices.device, "");
  push({ type: "applicationClose" });
  assert.equal(calls("save_settings").length, 0);
  discovery.resolve({
    devices: ["Mic"],
    outputDevices: ["Speakers"],
    audioAvailable: true,
    resolvedBackend: "cpal",
    deviceError: "",
  });
  await switching;
  assert.equal(calls("close_application").length, 1);
  assert.equal(calls("save_settings")[0].arguments[1].settings.devices.sampleRate, 44100);
});
test("close during manual saving reuses the completed save", async () => {
  await connect("settings").ready;
  await tick();
  const saving = deferred();
  handlers.set("save_settings", () => saving.promise);
  const action = settingsListener.saveSettings();
  push({ type: "close" });
  saving.resolve();
  await action;
  assert.equal(calls("save_settings").length, 1);
  assert.equal(calls("close_settings").length, 1);
});
test("failed save preserves draft, cancels shutdown and permits an explicit retry", async () => {
  await connect("settings").ready;
  await tick();
  settingsStore.getState().voice.setPitch(Number.NaN);
  handlers.set("save_settings", async () => {
    throw new Error("invalid pitch");
  });
  push({ type: "applicationClose" });
  await tick();
  assert.equal(calls("close_application").length, 0);
  assert.equal(calls("cancel_application_close").length, 1);
  assert.equal(Number.isNaN(settingsStore.getState().voice.pitch), true);
  assert.equal(useSettingsConnection.getState().status, "ready");
  handlers.delete("save_settings");
  settingsStore.getState().voice.setPitch(4);
  await settingsListener.saveSettings();
  assert.equal(calls("close_application").length, 0);
});
test("close queued during loading saves the snapshot and load failures allow hiding without saving", async () => {
  const loading = deferred();
  handlers.set("settings_view", () => loading.promise);
  await connect("settings").ready;
  push({ type: "close" });
  loading.resolve(settingsView);
  await tick();
  assert.equal(calls("save_settings").length, 1);
  assert.equal(calls("close_settings").length, 1);
  handlers.set("settings_view", async () => {
    throw new Error("load failed");
  });
  await connect("settings").ready;
  await tick();
  push({ type: "close" });
  await tick();
  assert.equal(calls("save_settings").length, 1);
  assert.equal(calls("close_settings").length, 2);
});
test("application close during load failure exits without saving a draft", async () => {
  handlers.set("settings_view", async () => {
    throw new Error("load failed");
  });
  await connect("settings").ready;
  await tick();
  push({ type: "applicationClose" });
  await tick();
  assert.equal(calls("save_settings").length, 0);
  assert.equal(calls("cancel_application_close").length, 0);
  assert.equal(calls("close_application").length, 1);
});
test("failed application close during load failure cancels the pending shutdown", async () => {
  handlers.set("settings_view", async () => {
    throw new Error("load failed");
  });
  handlers.set("close_application", async () => {
    throw new Error("cannot write settings");
  });
  await connect("settings").ready;
  await tick();
  push({ type: "applicationClose" });
  await tick();
  assert.equal(calls("close_application").length, 1);
  assert.equal(calls("cancel_application_close").length, 1);
  assert.match(useSettingsConnection.getState().error, /cannot write settings/);
});
test("old snapshot, notifications and saves cannot overwrite or hide a new connection", async () => {
  const loading = deferred();
  handlers.set("settings_view", () => loading.promise);
  await connect("settings").ready;
  const oldChannel = channels.get("settings");
  handlers.delete("settings_view");
  await connect("settings").ready;
  await tick();
  settingsStore.getState().voice.setPitch(6);
  loading.resolve(settingsView);
  oldChannel.onmessage({ type: "phase", phase: { phase: "running", busy: true, error: null } });
  await tick();
  assert.equal(settingsStore.getState().voice.pitch, 6);
  assert.equal(useSettingsConnection.getState().audioBusy, false);
  const saving = deferred();
  handlers.set("save_settings", () => saving.promise);
  push({ type: "close" });
  await connect("settings").ready;
  await tick();
  saving.resolve();
  await tick();
  assert.equal(calls("close_settings").length, 0);
});
test("routing and EQ setters do not retain caller-owned mutable arrays", () => {
  const outputs = [1, 2];
  settingsStore.getState().devices.setRouting(2, outputs);
  outputs.push(3);
  assert.deepEqual(settingsStore.getState().devices.outputChannels, [1, 2]);
  const bands = settings.eq.bands.map((band) => ({ ...band }));
  settingsStore.getState().eq.setBands(bands);
  bands[0].gainDb = 12;
  assert.equal(settingsStore.getState().eq.bands[0].gainDb, 0);
});

test("preview output is saved separately and cleared when switching backends", async () => {
  settingsStore.getState().devices.setPreviewDevice("Headphones");
  await settingsStore.getState().save();
  const saved = calls("save_settings").at(-1).arguments[1].settings;
  assert.equal(saved.devices.previewDevice, "Headphones");
  assert.equal(saved.devices.outputDevice, settingsStore.getState().devices.outputDevice);
  await settingsListener.changeBackend("asio");
  assert.equal(settingsStore.getState().devices.previewDevice, "");
});
