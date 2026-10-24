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

/** Apply the saved preference and keep it in sync across windows / OS changes. */
export function connectTheme() {
  syncThemeFromStorage();

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onMedia = () => {
    if (readThemePreference() === "system") {
      applyTheme(resolveTheme("system", media.matches));
    }
  };
  media.addEventListener("change", onMedia);

  const onStorage = (event: StorageEvent) => {
    if (event.key === themeStorageKey) {
      syncThemeFromStorage(event.newValue);
    }
  };
  window.addEventListener("storage", onStorage);

  return () => {
    media.removeEventListener("change", onMedia);
    window.removeEventListener("storage", onStorage);
  };
}
