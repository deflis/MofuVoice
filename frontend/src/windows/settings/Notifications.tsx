import { translateError } from "../../i18n/translateError";
import { useTranslation } from "react-i18next";
import styles from "./settings.module.css";
import { useSettingsConnection } from "./listener";

export function Notifications() {
  const { t } = useTranslation();
  const error = useSettingsConnection((state) => state.error || state.deviceError);
  const notice = useSettingsConnection((state) => state.notice);
  return (
    <>
      {error && (
        <div className={styles.error} role="alert">
          {translateError(error, t)}
        </div>
      )}
      {notice && <output className={styles.notice}>{t(notice)}</output>}
    </>
  );
}
