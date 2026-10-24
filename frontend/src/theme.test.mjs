import assert from "node:assert/strict";
import { test } from "node:test";
import { registerHooks } from "node:module";

const frontendRoot = new URL("./", import.meta.url).href;
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

const storage = new Map();
const documentElement = { dataset: {}, style: {} };

globalThis.localStorage = {
  getItem(key) {
    return storage.has(key) ? storage.get(key) : null;
  },
  setItem(key, value) {
    storage.set(key, value);
  },
  removeItem(key) {
    storage.delete(key);
  },
};

globalThis.window = {
  matchMedia() {
    return {
      matches: false,
      addEventListener() {},
      removeEventListener() {},
    };
  },
};

globalThis.document = {
  documentElement,
};

const {
  applyTheme,
  bootTheme,
  isThemePreference,
  readThemePreference,
  resolveTheme,
  setThemePreference,
  themePreferences,
  themeStorageKey,
} = await import("./theme.ts");

test("theme preferences are the supported appearance modes", () => {
  assert.deepEqual([...themePreferences], ["system", "light", "dark"]);
  assert.equal(isThemePreference("system"), true);
  assert.equal(isThemePreference("sepia"), false);
});

test("readThemePreference falls back to system for missing or invalid values", () => {
  assert.equal(readThemePreference(null), "system");
  assert.equal(readThemePreference("nope"), "system");
  assert.equal(readThemePreference("dark"), "dark");
});

test("resolveTheme maps system to the OS preference", () => {
  assert.equal(resolveTheme("light", true), "light");
  assert.equal(resolveTheme("dark", false), "dark");
  assert.equal(resolveTheme("system", true), "dark");
  assert.equal(resolveTheme("system", false), "light");
});

test("bootTheme applies the resolved preference from the module", () => {
  storage.set(themeStorageKey, "dark");
  bootTheme();
  assert.equal(documentElement.dataset.theme, "dark");
  assert.equal(documentElement.style.colorScheme, "dark");

  setThemePreference("light");
  assert.equal(documentElement.dataset.theme, "light");
  assert.equal(storage.get(themeStorageKey), "light");

  applyTheme("dark");
  assert.equal(documentElement.dataset.theme, "dark");
});
