import styles from "./settings.module.css";
import { useSettingsConnection } from "./listener";

export function Notifications() {
  const error = useSettingsConnection((state) => state.error || state.deviceError);
  const notice = useSettingsConnection((state) => state.notice);
  return (
    <>
      {error && (
        <div className={styles.error} role="alert">
          {error}
        </div>
      )}
      {notice && <output className={styles.notice}>{notice}</output>}
    </>
  );
}
