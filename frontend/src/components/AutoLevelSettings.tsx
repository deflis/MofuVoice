import { DetailsPopup } from "./DetailsPopup";
import type { LevelSettings } from "../settingsStore";
import { useAutoLevelSettings } from "../settingsHooks";
const levelFields: {
  key: Exclude<keyof LevelSettings, "enabled">;
  label: string;
  min: number;
  max: number;
}[] = [
  { key: "targetDb", label: "目標音量（dBFS・RMS）", min: -36, max: -6 },
  { key: "maxGainDb", label: "最大増幅量（dB）", min: 0, max: 18 },
  { key: "floorDb", label: "音声検出しきい値（dBFS）", min: -90, max: -30 },
  { key: "responseMs", label: "音量調整の応答時間（ms）", min: 100, max: 10000 },
  { key: "thresholdDb", label: "圧縮しきい値（dBFS）", min: -36, max: 0 },
  { key: "ratio", label: "圧縮比", min: 1, max: 20 },
  { key: "attackMs", label: "アタック（ms）", min: 0.1, max: 100 },
  { key: "releaseMs", label: "リリース（ms）", min: 10, max: 2000 },
];

export function AutoLevelSettings() {
  const { level, levelDefaults, updateLevel, resetLevel } = useAutoLevelSettings();
  return (
    <section>
      <h2>音量の自動調整</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={level.enabled}
          onChange={(event) => updateLevel("enabled", event.target.checked)}
        />
        有効
      </label>
      <p>小さい声と大きい声の音量を揃えます。</p>
      <fieldset className="settings" disabled={!level.enabled} hidden={!level.enabled}>
        <DetailsPopup title="音量の自動調整">
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
            標準に戻す
          </button>
        </DetailsPopup>
      </fieldset>
    </section>
  );
}
