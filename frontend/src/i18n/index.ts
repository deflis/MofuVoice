import { createInstance } from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json" with { type: "json" };
import ja from "./locales/ja.json" with { type: "json" };
import { languageKey, readLocale, type Locale } from "./locale";

export const i18n = createInstance();
// Bundled resources initialize synchronously, before any window renders.
void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, ja: { translation: ja } },
  lng: readLocale(),
  fallbackLng: "en",
  supportedLngs: ["ja", "en"],
  initAsync: false,
  keySeparator: false,
  nsSeparator: false,
  interpolation: { escapeValue: false }, // React escapes rendered text.
});

export function setLocale(locale: Locale) {
  try {
    globalThis.localStorage?.setItem(languageKey, locale);
  } catch {
    // Keep language switching usable when persistent storage is unavailable.
  }
  void i18n.changeLanguage(locale);
}

function onStorage(event: StorageEvent) {
  if (event.key === languageKey || event.key === null) {
    void i18n.changeLanguage(readLocale());
  }
}
export function connectLanguageStorage() {
  window.addEventListener("storage", onStorage);
  return () => window.removeEventListener("storage", onStorage);
}
