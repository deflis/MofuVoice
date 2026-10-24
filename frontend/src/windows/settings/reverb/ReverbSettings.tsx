import styles from "../settings.module.css";
import { useSettingsStore } from "../store";
import { NumberField } from "../NumberField";
export function ReverbSettings() {
  const values = useSettingsStore((state) => state.reverb);
  return (
    <section>
      <h2>リバーブ</h2>
      <p>残響を加えます。停止すると残響はリセットされます。</p>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={values.enabled}
          onChange={(event) => values.setEnabled(event.target.checked)}
        />
        有効
      </label>
      {!values.enabled && <p>バイパス中。設定値は保持されます。</p>}
      <NumberField
        label="ウェット比率"
        value={values.mix}
        onChange={values.setMix}
        min={0}
        max={1}
        step="any"
      />
      <NumberField
        label="プリディレイ（ms）"
        value={values.preDelayMs}
        onChange={values.setPreDelayMs}
        min={0}
        max={200}
        step="any"
      />
      <NumberField
        label="減衰時間（ms・公称RT60）"
        value={values.decayMs}
        onChange={values.setDecayMs}
        min={100}
        max={10000}
        step="any"
      />
    </section>
  );
}
