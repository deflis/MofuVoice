import styles from "./Home.module.css";
import type { SettingsSection } from "../settings/navigation";
import { openSpectrum, openSettings, openLog } from "./listener";
import { useMainStore } from "./store";
import { canOpenSpectrum, setupGuidance } from "./selectors";
import { AudioControls } from "./AudioControls";
import { AppHeader } from "./AppHeader";

export function Home() {
  const spectrumAvailable = useMainStore(canOpenSpectrum);
  return (
    <div className={styles["home-layout"]}>
      <AppHeader />
      <AudioControls />
      <div className={styles["home-settings-actions"]}>
        <OpenSettingsButton className={styles["home-primary-action"]} section="voice">
          声を調整する
        </OpenSettingsButton>
        <button
          type="button"
          className={styles["home-secondary-action"]}
          onClick={() => void openLog()}
        >
          ログ
        </button>
        <button
          type="button"
          className={styles["home-secondary-action"]}
          disabled={!spectrumAvailable}
          onClick={() => void openSpectrum()}
        >
          スペアナ
        </button>
        <OpenSettingsButton className={styles["home-secondary-action"]} section="devices">
          設定
        </OpenSettingsButton>
      </div>
      <DeviceSetupGuidance />
    </div>
  );
}

function OpenSettingsButton({
  section,
  children,
  className,
}: {
  section: SettingsSection;
  children: string;
  className: string;
}) {
  return (
    <button type="button" className={className} onClick={() => void openSettings(section)}>
      {children}
    </button>
  );
}

function DeviceSetupGuidance() {
  const reason = useMainStore(setupGuidance);
  if (!reason) {
    return null;
  }
  return (
    <aside className={styles["setup-guidance"]}>
      <p>{reason}</p>
      <OpenSettingsButton className={styles["home-secondary-action"]} section="devices">
        入力・出力デバイスを確認する
      </OpenSettingsButton>
    </aside>
  );
}
