import { useSettingsStore } from "../store";
import type { EqKey } from "./slice";
export const useEqSettings = (key: EqKey = "eq") => useSettingsStore((state) => state[key]);
