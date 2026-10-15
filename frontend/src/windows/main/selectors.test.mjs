import assert from "node:assert/strict";
import { test } from "node:test";
import { registerHooks } from "node:module";

const frontendRoot = new URL("../../", import.meta.url).href;
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      context.parentURL?.startsWith(frontendRoot) &&
      specifier.startsWith(".") &&
      !/\.[a-z]+$/.test(specifier)
    ) {
      return nextResolve(`${specifier}.ts`, context);
    }
    return nextResolve(specifier, context);
  },
});

const { canStart, canStop, canOpenSpectrum, headerLabel, setupGuidance } =
  await import("./selectors.ts");

const view = {
  canStart: true,
  spectrumAvailable: true,
  setup: {
    audioAvailable: true,
    resolvedBackend: "asio",
    inputConfigured: true,
    outputConfigured: true,
    rememberedDeviceMissing: false,
  },
  phase: { phase: "ready", error: null, busy: false },
};

test("selectors derive controls from machine status", () => {
  assert.equal(canStart({ status: "idle", view }), true);
  assert.equal(canStart({ status: "idle", view: { ...view, canStart: false } }), false);
  assert.equal(canStart({ status: "faulted", view, error: "x" }), true);
  assert.equal(canStart({ status: "running", view }), false);
  assert.equal(canStop({ status: "running", view }), true);
  assert.equal(canOpenSpectrum({ status: "faulted", view, error: "x" }), true);
  assert.equal(headerLabel({ status: "running", view }), "音声処理中");
});

test("setup guidance uses main view facts only", () => {
  assert.equal(setupGuidance(view), "");
  assert.match(
    setupGuidance({
      ...view,
      setup: { ...view.setup, audioAvailable: false },
    }),
    /音声機器/,
  );
  assert.match(
    setupGuidance({
      ...view,
      setup: { ...view.setup, inputConfigured: false },
    }),
    /デバイスを設定/,
  );
});
