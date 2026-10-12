import { useEffect, useReducer } from "react";
import { useSettingsStore } from "./settingsStore";
import { useSettingsLoadFailed, useSettingsReady } from "./settingsHooks";
import { AudioControls } from "./components/AudioControls";
import { SettingsPanel } from "./components/SettingsPanel";
import { SaveSettingsButton } from "./components/SaveSettingsButton";
import { AppHeader, Notifications, StatusDetails } from "./components/Status";

function SettingsConnection() {
  useEffect(() => useSettingsStore.getState().connect(), []);
  return null;
}

export function App() {
  const ready = useSettingsReady();
  const loadFailed = useSettingsLoadFailed();
  const [connectionAttempt, retryConnection] = useReducer((attempt: number) => attempt + 1, 0);

  return (
    <main>
      <SettingsConnection key={connectionAttempt} />
      <AppHeader />
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
          <AudioControls />
          <SettingsPanel />
          <SaveSettingsButton />
          <StatusDetails />
        </>
      )}
    </main>
  );
}
