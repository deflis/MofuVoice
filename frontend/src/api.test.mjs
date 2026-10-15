import assert from "node:assert/strict";
import { after, beforeEach, mock, test } from "node:test";
import { registerTypeScript } from "./testing/registerTypeScript.mjs";

registerTypeScript();

const invoke = mock.fn(async () => undefined);
const listen = mock.fn(async () => () => {});
mock.module("@tauri-apps/api/core", { exports: { invoke } });
mock.module("@tauri-apps/api/event", { exports: { listen } });
const { mainApi } = await import("./windows/main/api.ts");
const { settingsApi } = await import("./windows/settings/api.ts");
const { logApi } = await import("./windows/log/api.ts");
const storageDescriptor = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
const setItem = mock.fn();
Object.defineProperty(globalThis, "localStorage", { configurable: true, value: { setItem } });
after(() => {
  if (storageDescriptor) {
    Object.defineProperty(globalThis, "localStorage", storageDescriptor);
  } else {
    delete globalThis.localStorage;
  }
});
beforeEach(() => {
  invoke.mock.resetCalls();
  listen.mock.resetCalls();
  setItem.mock.resetCalls();
});

test("main API preserves commands and sends no draft settings on start or close", async () => {
  await mainApi.getView();
  await mainApi.start(false);
  await mainApi.start(true);
  await mainApi.stop();
  await mainApi.spectrum();
  await mainApi.openSettings("voice");
  await mainApi.openLog();
  await mainApi.close();
  assert.deepEqual(
    invoke.mock.calls.map((call) => call.arguments),
    [
      ["main_view"],
      ["start_audio", { inspect: false }],
      ["start_audio", { inspect: true }],
      ["stop_audio"],
      ["open_spectrum"],
      ["open_settings"],
      ["open_log"],
      ["close_application"],
    ],
  );
  assert.equal(setItem.mock.calls[0].arguments[0], "voice-changer.settings-navigation");
  const navigation = JSON.parse(setItem.mock.calls[0].arguments[1]);
  assert.equal(navigation.section, "voice");
  assert.equal(typeof navigation.request, "string");
});

test("settings API preserves settings, backend, and close delegation payloads", async () => {
  const settings = { pitch: "3" };
  await settingsApi.getView();
  await settingsApi.save(settings);
  await settingsApi.deviceChoices("cpal");
  await settingsApi.deviceSampleRates({
    backend: "cpal",
    device: "Mic",
    outputDevice: "Headphones",
  });
  await settingsApi.hide();
  await settingsApi.acknowledgeClose(7);
  await settingsApi.fallbackClose(7);
  await settingsApi.closeApplication();
  assert.deepEqual(
    invoke.mock.calls.map((call) => call.arguments),
    [
      ["settings_view"],
      ["save_settings", { settings }],
      ["device_choices", { backend: "cpal" }],
      ["device_sample_rates", { backend: "cpal", device: "Mic", outputDevice: "Headphones" }],
      ["close_settings"],
      ["acknowledge_application_close", { requestId: 7 }],
      ["fallback_application_close", { requestId: 7 }],
      ["close_application"],
    ],
  );
});

test("log API preserves logging enable and disable payloads", async () => {
  await logApi.getView();
  await logApi.setLogging(true);
  await logApi.setLogging(false);
  await logApi.hide();
  assert.deepEqual(
    invoke.mock.calls.map((call) => call.arguments),
    [
      ["log_view"],
      ["set_logging", { logging: true }],
      ["set_logging", { logging: false }],
      ["close_log"],
    ],
  );
});

test("API subscriptions expose payloads and return the original cleanup", async () => {
  const subscriptions = [
    [mainApi.onPhase, "audio-phase"],
    [mainApi.onView, "main-view"],
    [mainApi.onClose, "request-close"],
    [settingsApi.onPhase, "audio-phase"],
    [settingsApi.onClose, "request-close"],
    [settingsApi.onApplicationClose, "request-application-close"],
    [logApi.onInfo, "log-info"],
    [logApi.onLog, "audio-log"],
    [logApi.onClose, "request-close"],
  ];
  for (const [subscribe, event] of subscriptions) {
    const remove = mock.fn();
    listen.mock.mockImplementationOnce(async () => remove);
    const receive = mock.fn();
    assert.equal(await subscribe(receive), remove);
    const call = listen.mock.calls.at(-1);
    assert.equal(call.arguments[0], event);
    const payload = { value: 7 };
    call.arguments[1]({ payload });
    assert.equal(receive.mock.calls[0].arguments[0], payload);
  }
});
