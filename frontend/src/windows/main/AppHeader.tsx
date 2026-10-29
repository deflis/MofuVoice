import { LanguageSelect } from "../../i18n/LanguageSelect";
import { useTranslation } from "react-i18next";
import styles from "./AppHeader.module.css";
import { useMainStore } from "./store";
import { headerLabel } from "./selectors";

export function AppHeader() {
  const { t } = useTranslation();
  const status = useMainStore((state) => headerLabel(state));
  return (
    <header>
      <h1>{t("もふボイス")}</h1>
      <output className={styles["status-badge"]}>{t(status)}</output>
      <LanguageSelect />
    </header>
  );
}
