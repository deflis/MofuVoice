import type { SettingsSection } from "../settings/navigation";
import { useMainStore } from "./store";
import { canOpenSpectrum, setupGuidance } from "./selectors";
import { AudioControls } from "./AudioControls";
import { AppHeader } from "./AppHeader";

export function Home() {
  const machine = useMainStore((state) => state.machine);
  const spectrum = useMainStore((state) => state.spectrum);
  const openLog = useMainStore((state) => state.openLog);
  if (!("view" in machine)) {
    throw new Error("メイン画面の表示状態が不正です。");
  }
  return (
    <div className="home-layout">
      <AppHeader />
      <AudioControls />
      <div className="home-settings-actions">
        <OpenSettingsButton className="home-primary-action" section="voice">
          声を調整する
        </OpenSettingsButton>
        <button type="button" className="home-secondary-action" onClick={() => void openLog()}>
          ログ
        </button>
        <button
          type="button"
          className="home-secondary-action"
          disabled={!canOpenSpectrum(machine)}
          onClick={() => void spectrum()}
        >
          スペアナ
        </button>
        <OpenSettingsButton className="home-secondary-action" section="devices">
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
  const openSettings = useMainStore((state) => state.openSettings);
  return (
    <button type="button" className={className} onClick={() => void openSettings(section)}>
      {children}
    </button>
  );
}

function DeviceSetupGuidance() {
  const reason = useMainStore((state) => {
    const { machine } = state;
    if (!("view" in machine)) {
      return "";
    }
    return setupGuidance(machine.view);
  });
  if (!reason) {
    return null;
  }
  return (
    <aside className="setup-guidance">
      <p>{reason}</p>
      <OpenSettingsButton className="home-secondary-action" section="devices">
        入力・出力デバイスを確認する
      </OpenSettingsButton>
    </aside>
  );
}
