import { LanguageSelect } from "../../i18n/LanguageSelect";
import { useTranslation } from "react-i18next";
import styles from "./App.module.css";
import commonStyles from "./log.module.css";
import { Notifications } from "./Notifications";
import { StatusDetails } from "./StatusDetails";

export function LogApp() {
  const { t } = useTranslation();
  return (
    <main>
      <Notifications />
      <div className={styles["log-window"]}>
        <header>
          <h1>{t("ログ")}</h1>
          <span className={commonStyles.muted}>{t("もふボイス")}</span>
          <LanguageSelect />
        </header>
        <StatusDetails />
      </div>
    </main>
  );
}
