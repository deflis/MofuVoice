import styles from "./Notifications.module.css";
import { useMainStore } from "./store";

export function Notifications() {
  const error = useMainStore((state) => state.error);
  return (
    <>
      {error && (
        <div className={styles.error} role="alert">
          {error}
        </div>
      )}
    </>
  );
}
