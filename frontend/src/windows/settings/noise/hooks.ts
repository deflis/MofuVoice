import { useSettingsStore } from "../store";
export const useNoiseGateSettings = () => useSettingsStore((state) => state.noise);
