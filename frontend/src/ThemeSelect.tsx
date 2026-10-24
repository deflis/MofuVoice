import { useEffect, useState } from "react";
import {
  isThemePreference,
  readThemePreference,
  setThemePreference,
  themePreferenceLabels,
  themePreferences,
  themeStorageKey,
  type ThemePreference,
} from "./theme";
import styles from "./ThemeSelect.module.css";

export function ThemeSelect({ id = "theme-preference" }: { id?: string }) {
  const [preference, setPreference] = useState<ThemePreference>(() => readThemePreference());

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === themeStorageKey) {
        setPreference(readThemePreference(event.newValue));
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  return (
    <label className={styles["theme-select"]} htmlFor={id}>
      <span className={styles["theme-select-label"]}>外観</span>
      <select
        id={id}
        value={preference}
        onChange={(event) => {
          const next = event.target.value;
          if (!isThemePreference(next)) {
            return;
          }
          setThemePreference(next);
          setPreference(next);
        }}
      >
        {themePreferences.map((candidate) => (
          <option key={candidate} value={candidate}>
            {themePreferenceLabels[candidate]}
          </option>
        ))}
      </select>
    </label>
  );
}
