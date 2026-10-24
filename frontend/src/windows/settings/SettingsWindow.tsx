import styles from "./SettingsWindow.module.css";
import commonStyles from "./settings.module.css";
import { SettingsPanel } from "./SettingsPanel";
import { SaveSettingsButton } from "./SaveSettingsButton";
import { useSettingsConnection } from "./listener";

export function SettingsWindow() {
  const busy = useSettingsConnection(
    (state) =>
      state.audioBusy ||
      state.status === "saving" ||
      state.status === "switchingBackend" ||
      state.status === "closing",
  );
  return (
    <div className={styles["settings-window"]}>
      <header>
        <h1>設定</h1>
        <span className={commonStyles.muted}>もふボイス</span>
      </header>
      {busy && <output>音声を停止すると設定を変更できます。</output>}
      <SettingsPanel />
      <SaveSettingsButton />
    </div>
  );
}
