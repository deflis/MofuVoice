import type { LevelSettings } from "../settingsStore";
import { useAutoLevelSettings } from "../settingsHooks";
const levelFields: {
  key: Exclude<keyof LevelSettings, "enabled">;
  label: string;
  min: number;
  max: number;
}[] = [
  { key: "targetDb", label: "Target loudness (LUFS)", min: -36, max: -6 },
  { key: "maxGainDb", label: "Max gain (dB)", min: 0, max: 18 },
  { key: "floorDb", label: "Activity floor (dBFS)", min: -90, max: -30 },
  { key: "responseMs", label: "Level response (ms)", min: 100, max: 10000 },
  { key: "thresholdDb", label: "Comp. threshold (dBFS)", min: -36, max: 0 },
  { key: "ratio", label: "Ratio", min: 1, max: 20 },
  { key: "attackMs", label: "Attack (ms)", min: 0.1, max: 100 },
  { key: "releaseMs", label: "Release (ms)", min: 10, max: 2000 },
];

export function AutoLevelSettings() {
  const { level, levelDefaults, updateLevel, resetLevel } = useAutoLevelSettings();
  return (
    <section>
      <h2>Auto level</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={level.enabled}
          onChange={(event) => updateLevel("enabled", event.target.checked)}
        />
        有効
      </label>
      <details>
        <summary>詳細設定</summary>
        <div className="level-fields">
          {levelFields.map((field) => (
            <label key={field.key}>
              {field.label}
              <input
                type="number"
                step="any"
                required
                min={field.min}
                max={field.max}
                value={level[field.key]}
                onChange={(event) => updateLevel(field.key, event.target.value)}
              />
            </label>
          ))}
        </div>
        <button type="button" disabled={!levelDefaults} onClick={resetLevel}>
          既定値に戻す
        </button>
      </details>
    </section>
  );
}
