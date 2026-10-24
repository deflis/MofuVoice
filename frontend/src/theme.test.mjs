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

const { isThemePreference, readThemePreference, resolveTheme, themePreferences } =
  await import("./theme.ts");

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
