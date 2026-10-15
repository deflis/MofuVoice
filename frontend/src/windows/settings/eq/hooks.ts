import { useSettingsStore } from "../store";
export const useEqSettings = () => useSettingsStore((state) => state.eq);
