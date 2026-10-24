import styles from "../settings.module.css";
import { useSettingsStore } from "../store";
import { NumberField } from "../NumberField";
export function LimiterSettings() {
  const values = useSettingsStore((state) => state.limiter);
  return (
    <section>
      <h2>リミッター</h2>
      <p>瞬間的なピークを抑えます。後段の処理によるピークは制限しません。</p>
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
        label="上限（dBFS）"
        value={values.ceilingDb}
        onChange={values.setCeilingDb}
        min={-24}
        max={0}
        step="any"
      />
      <NumberField
        label="リリース（ms）"
        value={values.releaseMs}
        onChange={values.setReleaseMs}
        min={1}
        max={2000}
        step="any"
      />
    </section>
  );
}
