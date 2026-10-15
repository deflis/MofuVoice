import { useSettingsStore } from "../store";
export const useDeEsserSettings = () => useSettingsStore((state) => state.deesser);
