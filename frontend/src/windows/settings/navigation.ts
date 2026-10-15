export type SettingsSection = "devices" | "voice" | "noise" | "tone";
const navigationKey = "voice-changer.settings-navigation";

export function readSettingsSection(value = localStorage.getItem(navigationKey)): SettingsSection {
  try {
    const section: unknown = JSON.parse(value ?? "null")?.section;
    if (section === "devices" || section === "voice" || section === "noise" || section === "tone") {
      return section;
    }
  } catch {
    // A previous or invalid navigation value should not block opening settings.
  }
  return "voice";
}

export function selectSettingsSection(section: SettingsSection) {
  // Shared WebView storage also delivers navigation when the OS window is already open.
  // A unique request lets the same shortcut work after navigating elsewhere in settings.
  localStorage.setItem(navigationKey, JSON.stringify({ section, request: crypto.randomUUID() }));
}

export function connectSettingsNavigation(select: (section: SettingsSection) => void) {
  const onStorage = (event: StorageEvent) => {
    if (event.key === navigationKey) {
      select(readSettingsSection(event.newValue));
    }
  };
  window.addEventListener("storage", onStorage);
  return () => window.removeEventListener("storage", onStorage);
}
