import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import { useNoiseGateSettings } from "./hooks";

export function RumbleSettings() {
  const { t } = useTranslation();
  const { rumbleFilter, setRumbleFilter } = useNoiseGateSettings();
  return (
    <section>
      <h2>{t("入力EQのハイパス（HPF）")}</h2>
      <p>
        {t(
          "80 Hz以下の低域を抑え、マイクの吹かれ・振動ノイズを減らします。EQの帯域調整とは個別に切り替えられます。",
        )}
      </p>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={rumbleFilter}
          onChange={(event) => setRumbleFilter(event.target.checked)}
        />
        {t("有効")}
      </label>
    </section>
  );
}
