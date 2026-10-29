import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import { useSettingsStore } from "../store";
import { NumberField } from "../NumberField";
export function DereverbSettings() {
  const { t } = useTranslation();
  const values = useSettingsStore((state) => state.dereverb);
  return (
    <section>
      <h2>{t("反響低減")}</h2>
      <p>
        {t(
          "音量の減衰部分を抑える簡易方式です。直接音と残響を分離する処理ではなく、語尾も弱まることがあります。",
        )}
      </p>
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
        label={t("抑制量")}
        value={values.amount}
        onChange={values.setAmount}
        min={0}
        max={1}
        step="any"
      />
      <NumberField
        label={t("減衰時間（ms）")}
        value={values.decayMs}
        onChange={values.setDecayMs}
        min={20}
        max={2000}
        step="any"
      />
    </section>
  );
}
