import styles from "../settings.module.css";
import { useSettingsStore } from "../store";
import { NumberField } from "../NumberField";
export function DereverbSettings() {
  const values = useSettingsStore((state) => state.dereverb);
  return (
    <section>
      <h2>反響低減</h2>
      <p>
        音量の減衰部分を抑える簡易方式です。直接音と残響を分離する処理ではなく、語尾も弱まることがあります。
      </p>
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
        label="抑制量"
        value={values.amount}
        onChange={values.setAmount}
        min={0}
        max={1}
        step="any"
      />
      <NumberField
        label="減衰時間（ms）"
        value={values.decayMs}
        onChange={values.setDecayMs}
        min={20}
        max={2000}
        step="any"
      />
    </section>
  );
}
