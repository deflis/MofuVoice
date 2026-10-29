import assert from "node:assert/strict";
import { test, after } from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { registerTypeScript } from "../testing/registerTypeScript.mjs";
registerTypeScript();
const { resolveLocale, languageKey } = await import("./locale.ts");
const { i18n, setLocale, connectLanguageStorage } = await import("./index.ts");
const { translateError } = await import("./translateError.ts");
const en = JSON.parse(readFileSync(new URL("./locales/en.json", import.meta.url)));
const ja = JSON.parse(readFileSync(new URL("./locales/ja.json", import.meta.url)));

test("saved language wins; regional system languages and unsupported values fall back safely", () => {
  assert.equal(resolveLocale("en", ["ja-JP"]), "en");
  assert.equal(resolveLocale("ja", ["en-US"]), "ja");
  assert.equal(resolveLocale(null, ["ja-JP", "en-US"]), "ja");
  assert.equal(resolveLocale("invalid", ["fr-FR", "EN-gb"]), "en");
  assert.equal(resolveLocale(null, ["fr", "ja"]), "ja");
  assert.equal(resolveLocale(null, []), "en");
});

const variables = (value) =>
  [...value.matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).toSorted((a, b) => a.localeCompare(b));

test("both resources cover every key with identical interpolation variables", () => {
  assert.deepEqual(Object.keys(en).toSorted(), Object.keys(ja).toSorted());
  for (const key of Object.keys(ja)) {
    assert.ok(en[key].trim(), key);

    assert.deepEqual(variables(en[key]), variables(ja[key]), key);
    assert.doesNotMatch(en[key], /[ぁ-んァ-ヶ一-龠]/, key);
  }
});

test("i18next translates interpolation and already visible validation errors in either language", () => {
  for (const locale of ["en", "ja", "en"]) {
    setLocale(locale);
    assert.equal(i18n.t("声の変換"), locale === "en" ? "Voice conversion" : "声の変換");
    assert.equal(i18n.t("バンド{0}", { v0: 3 }), locale === "en" ? "Band 3" : "バンド3");
    assert.equal(
      translateError("ピッチは-12〜12の数値を入力してください。", i18n.t),
      locale === "en"
        ? "Enter a number from -12 to 12 for Pitch."
        : "ピッチは-12〜12の数値を入力してください。",
    );
    assert.equal(
      translateError("入力チャンネルには整数を入力してください。", i18n.t),
      locale === "en"
        ? "Enter an integer for Input channel."
        : "入力チャンネルには整数を入力してください。",
    );
    assert.equal(i18n.t("Driver error: <device>&"), "Driver error: <device>&");
  }
});

test("language persists, synchronizes across windows and works without storage", () => {
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  const values = new Map();
  const target = new EventTarget();
  Object.defineProperty(globalThis, "window", { configurable: true, value: target });
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => values.set(key, value),
    },
  });
  const close = connectLanguageStorage();
  try {
    setLocale("ja");
    assert.equal(values.get(languageKey), "ja");
    values.set(languageKey, "en");
    const event = new Event("storage");
    Object.defineProperty(event, "key", { value: languageKey });
    target.dispatchEvent(event);
    assert.equal(i18n.resolvedLanguage, "en");
    close();
    values.set(languageKey, "ja");
    target.dispatchEvent(event);
    assert.equal(i18n.resolvedLanguage, "en");
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() {
        throw new Error("blocked");
      },
    });
    setLocale("ja");
    assert.equal(i18n.resolvedLanguage, "ja");
  } finally {
    close();
    if (originalWindow) {
      Object.defineProperty(globalThis, "window", originalWindow);
    } else {
      delete globalThis.window;
    }
    if (originalStorage) {
      Object.defineProperty(globalThis, "localStorage", originalStorage);
    } else {
      delete globalThis.localStorage;
    }
  }
});

test("every literal translation used by the UI has a bundled resource", () => {
  const root = new URL("../windows/", import.meta.url);
  for (const file of readdirSync(root, { recursive: true }).filter((entry) =>
    entry.endsWith(".tsx"),
  )) {
    const source = readFileSync(new URL(file, root), "utf8");
    for (const match of source.matchAll(/\bt\(\s*("(?:[^"\\]|\\.)*")/g)) {
      assert.ok(Object.hasOwn(ja, JSON.parse(match[1])), `${file}: ${match[1]}`);
    }
  }
});
after(() => {
  void i18n.changeLanguage("en");
});
