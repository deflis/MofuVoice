import { translateError } from "../../i18n/translateError";
import { useTranslation } from "react-i18next";
import styles from "./Notifications.module.css";
import { useMainStore } from "./store";

export function Notifications() {
  const { t } = useTranslation();
  const error = useMainStore((state) => state.error);
  return (
    <>
      {error && (
        <div className={styles.error} role="alert">
          {translateError(error, t)}
        </div>
      )}
    </>
  );
}
