import { canEditSettings, useSettingsConnection, saveSettings } from "./listener";
export function useSettingsLocked() {
  return useSettingsConnection((state) => !canEditSettings(state));
}
export function useSaveSettings() {
  return { disabled: useSettingsLocked(), save: saveSettings };
}
