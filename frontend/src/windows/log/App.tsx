import styles from "../../styles.module.css";
import { Notifications } from "./Notifications";
import { StatusDetails } from "./StatusDetails";

export function LogApp() {
  return (
    <main>
      <Notifications />
      <div className={styles["log-window"]}>
        <header>
          <h1>ログ</h1>
          <span className={styles.muted}>Voice Changer</span>
        </header>
        <StatusDetails />
      </div>
    </main>
  );
}
