import { useSettingsStore } from "../store";
export const useVoiceSettings = () => useSettingsStore((state) => state.voice);
