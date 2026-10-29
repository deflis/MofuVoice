import { translateError } from "../../i18n/translateError";
import { useTranslation } from "react-i18next";
import styles from "./log.module.css";
import { useLogStore } from "./store";

export function Notifications() {
  const { t } = useTranslation();
  const error = useLogStore((state) => state.error);
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
