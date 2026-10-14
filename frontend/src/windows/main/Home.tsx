import { openSettings, type SettingsSection } from "../../settingsNavigation";
import { openLog } from "../../logNavigation";
import { useSettingsStore } from "../../settingsStore";
import { useAudioStore } from "../../audioStore";
import { useAudioControls } from "../../audioHooks";
import { AudioControls } from "../../components/AudioControls";
import { AppHeader } from "../../components/AppHeader";

export function Home() {
  const { spectrumDisabled, spectrum } = useAudioControls();
  return (
    <div className="home-layout">
      <AppHeader />
      <AudioControls />
      <div className="home-settings-actions">
        <OpenSettingsButton className="home-primary-action" section="voice">
          声を調整する
        </OpenSettingsButton>
        <button
          type="button"
          className="home-secondary-action"
          onClick={() => {
            void openLog().catch((error: unknown) =>
              useAudioStore.setState({ error: String(error) }),
            );
          }}
        >
          ログ
        </button>
        <button
          type="button"
          className="home-secondary-action"
          disabled={spectrumDisabled}
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
  return (
    <button
      type="button"
      className={className}
      onClick={() => {
        void openSettings(section).catch((error: unknown) =>
          useAudioStore.setState({ error: String(error) }),
        );
      }}
    >
      {children}
    </button>
  );
}

function DeviceSetupGuidance() {
  const reason = useSettingsStore((state) => {
    const settings = state.settings;
    if (!settings || state.switchingBackend) {
      return "";
    }
    if (!state.audioAvailable) {
      return "使える音声機器が見つかりません。接続と設定を確認してください。";
    }
    if (state.resolvedBackend === "asio" && !settings.device) {
      return "最初に、入力・出力デバイスを設定してください。";
    }
    if (
      (settings.device && !state.devices.includes(settings.device)) ||
      (state.resolvedBackend === "cpal" &&
        settings.outputDevice &&
        !state.outputDevices.includes(settings.outputDevice))
    ) {
      return "前回使った音声機器が見つかりません。入力・出力デバイスを確認してください。";
    }
    return "";
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
