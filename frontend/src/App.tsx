import { openSettings, type SettingsSection } from "./settingsNavigation";
import { useSettingsStore } from "./settingsStore";
import { useAudioStore } from "./audioStore";
import { useEffect, useReducer } from "react";
import { connectApplication } from "./applicationConnection";
import { useSettingsReady, useSettingsLoadFailed } from "./settingsHooks";
import { AudioControls } from "./components/AudioControls";
import { SettingsWindow } from "./components/SettingsWindow";
import { AppHeader } from "./components/AppHeader";
import { Notifications } from "./components/Notifications";

function ApplicationConnection() {
  useEffect(() => connectApplication(settingsWindow).disconnect, []);
  return null;
}

const settingsWindow = new URLSearchParams(window.location.search).get("window") === "settings";

export function App() {
  const ready = useSettingsReady();
  const loadFailed = useSettingsLoadFailed();
  const [connectionAttempt, retryConnection] = useReducer((attempt: number) => attempt + 1, 0);

  return (
    <main>
      <ApplicationConnection key={connectionAttempt} />
      {!settingsWindow && <AppHeader />}
      <Notifications />
      {!ready ? (
        <div>
          {loadFailed ? (
            <button type="button" onClick={retryConnection}>
              設定の読み込みを再試行
            </button>
          ) : (
            <p>設定を読み込んでいます…</p>
          )}
        </div>
      ) : (
        <>
          {settingsWindow ? (
            <SettingsWindow />
          ) : (
            <>
              <AudioControls />
              <div className="home-settings-actions">
                <OpenSettingsButton section="voice">声を調整する</OpenSettingsButton>
                <OpenSettingsButton section="devices">設定</OpenSettingsButton>
              </div>
              <DeviceSetupGuidance />
            </>
          )}
        </>
      )}
    </main>
  );
}

function OpenSettingsButton({ section, children }: { section: SettingsSection; children: string }) {
  return (
    <button
      type="button"
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
      return "最初に、マイク・出力先を設定してください。";
    }
    if (
      (settings.device && !state.devices.includes(settings.device)) ||
      (state.resolvedBackend === "cpal" &&
        settings.outputDevice &&
        !state.outputDevices.includes(settings.outputDevice))
    ) {
      return "前回使った音声機器が見つかりません。マイク・出力先を確認してください。";
    }
    return "";
  });
  if (!reason) {
    return null;
  }
  return (
    <aside className="setup-guidance">
      <p>{reason}</p>
      <OpenSettingsButton section="devices">マイク・出力先を確認する</OpenSettingsButton>
    </aside>
  );
}
