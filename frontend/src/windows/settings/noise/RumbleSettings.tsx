import styles from "../../../styles.module.css";
import { useNoiseGateSettings } from "./hooks";

export function RumbleSettings() {
  const { rumbleFilter, setRumbleFilter } = useNoiseGateSettings();
  return (
    <section>
      <h2>低い唸りを減らす（低域ノイズ除去）</h2>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={rumbleFilter}
          onChange={(event) => setRumbleFilter(event.target.checked)}
        />
        有効
      </label>
    </section>
  );
}
