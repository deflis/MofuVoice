import { LanguageSelect } from "../../i18n/LanguageSelect";
import { useTranslation } from "react-i18next";
import styles from "./SettingsWindow.module.css";
import commonStyles from "./settings.module.css";
import { SettingsPanel } from "./SettingsPanel";
import { SaveSettingsButton } from "./SaveSettingsButton";
import { useSettingsConnection } from "./listener";

export function SettingsWindow() {
  const { t } = useTranslation();
  const busy = useSettingsConnection(
    (state) =>
      state.audioBusy ||
      state.status === "saving" ||
      state.status === "switchingBackend" ||
      state.status === "closing",
  );
  return (
    <div className={styles["settings-window"]}>
      <header>
        <h1>{t("設定")}</h1>
        <span className={commonStyles.muted}>{t("もふボイス")}</span>
        <LanguageSelect />
      </header>
      {busy && <output>{t("音声を停止すると設定を変更できます。")}</output>}
      <SettingsPanel />
      <SaveSettingsButton />
    </div>
  );
}
