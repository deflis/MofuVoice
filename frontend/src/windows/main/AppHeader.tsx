import styles from "../../styles.module.css";
import { useMainStore } from "./store";
import { headerLabel } from "./selectors";

export function AppHeader() {
  const status = useMainStore((state) => headerLabel(state));
  return (
    <header>
      <h1>Voice Changer</h1>
      <output className={styles["status-badge"]}>{status}</output>
    </header>
  );
}
