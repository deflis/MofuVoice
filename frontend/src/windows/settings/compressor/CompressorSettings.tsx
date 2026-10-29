import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import { useSettingsStore } from "../store";
import { NumberField } from "../NumberField";
export function CompressorSettings() {
  const { t } = useTranslation();
  const values = useSettingsStore((state) => state.compressor);
  return (
    <section>
      <h2>{t("前処理コンプレッサー")}</h2>
      <p>{t("声変換前の大きな声を抑えます。後処理の音量調整とは独立しています。")}</p>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={values.enabled}
          onChange={(event) => values.setEnabled(event.target.checked)}
        />
        {t("有効")}
      </label>
      {!values.enabled && <p>{t("バイパス中。設定値は保持されます。")}</p>}
      <NumberField
        label={t("しきい値（dBFS）")}
        value={values.thresholdDb}
        onChange={values.setThresholdDb}
        min={-36}
        max={0}
        step="any"
      />
      <NumberField
        label={t("圧縮比")}
        value={values.ratio}
        onChange={values.setRatio}
        min={1}
        max={20}
        step="any"
      />
      <NumberField
        label={t("アタック（ms）")}
        value={values.attackMs}
        onChange={values.setAttackMs}
        min={0.1}
        max={100}
        step="any"
      />
      <NumberField
        label={t("リリース（ms）")}
        value={values.releaseMs}
        onChange={values.setReleaseMs}
        min={10}
        max={2000}
        step="any"
      />
    </section>
  );
}
