export const themePreferences = ["system", "light", "dark"] as const;
export type ThemePreference = (typeof themePreferences)[number];
export type ResolvedTheme = "light" | "dark";

export const themeStorageKey = "mofu-voice.theme";

export const themePreferenceLabels: Record<ThemePreference, string> = {
  system: "システム",
  light: "ライト",
  dark: "ダーク",
};

export function isThemePreference(value: unknown): value is ThemePreference {
  return themePreferences.some((candidate) => candidate === value);
}

export function readThemePreference(
  value = localStorage.getItem(themeStorageKey),
): ThemePreference {
  if (isThemePreference(value)) {
    return value;
  }
  return "system";
}

export function resolveTheme(
  preference: ThemePreference,
  prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches,
): ResolvedTheme {
  if (preference === "light") {
    return "light";
  }
  if (preference === "dark") {
    return "dark";
  }
  return prefersDark ? "dark" : "light";
}

export function applyTheme(theme: ResolvedTheme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

export function setThemePreference(preference: ThemePreference) {
  localStorage.setItem(themeStorageKey, preference);
  applyTheme(resolveTheme(preference));
}

export function syncThemeFromStorage(value = localStorage.getItem(themeStorageKey)) {
  applyTheme(resolveTheme(readThemePreference(value)));
}

/** Resolve and apply the current preference (used by the theme module entry path). */
export function bootTheme() {
  syncThemeFromStorage();
}

function onThemeStorage(event: StorageEvent) {
  if (event.key === themeStorageKey) {
    syncThemeFromStorage(event.newValue);
  }
}

/** Apply the saved preference and keep it in sync across windows / OS changes. */
export function connectTheme() {
  bootTheme();

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onMedia = () => {
    if (readThemePreference() === "system") {
      applyTheme(resolveTheme("system", media.matches));
    }
  };
  media.addEventListener("change", onMedia);
  window.addEventListener("storage", onThemeStorage);

  return () => {
    media.removeEventListener("change", onMedia);
    window.removeEventListener("storage", onThemeStorage);
  };
}
