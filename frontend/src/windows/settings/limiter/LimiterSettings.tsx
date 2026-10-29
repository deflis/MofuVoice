import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import { useSettingsStore } from "../store";
import { NumberField } from "../NumberField";
export function LimiterSettings() {
  const { t } = useTranslation();
  const values = useSettingsStore((state) => state.limiter);
  return (
    <section>
      <h2>{t("リミッター")}</h2>
      <p>{t("瞬間的なピークを抑えます。後段の処理によるピークは制限しません。")}</p>
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
        label={t("上限（dBFS）")}
        value={values.ceilingDb}
        onChange={values.setCeilingDb}
        min={-24}
        max={0}
        step="any"
      />
      <NumberField
        label={t("リリース（ms）")}
        value={values.releaseMs}
        onChange={values.setReleaseMs}
        min={1}
        max={2000}
        step="any"
      />
    </section>
  );
}
