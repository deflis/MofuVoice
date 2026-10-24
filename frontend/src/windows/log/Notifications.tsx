import styles from "./log.module.css";
import { useLogStore } from "./store";

export function Notifications() {
  const error = useLogStore((state) => state.error);
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
