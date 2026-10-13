import { useEffect, useReducer } from "react";
import { connectApplication } from "./applicationConnection";
import { useSettingsReady, useSettingsLoadFailed } from "./settingsHooks";
import { AudioControls } from "./components/AudioControls";
import { SettingsWindow } from "./components/SettingsWindow";
import { AppHeader } from "./components/AppHeader";
import { Notifications } from "./components/Notifications";
import { StatusDetails } from "./components/StatusDetails";

function ApplicationConnection() {
  useEffect(() => connectApplication().disconnect, []);
  return null;
}

export function App() {
  const ready = useSettingsReady();
  const loadFailed = useSettingsLoadFailed();
  const [connectionAttempt, retryConnection] = useReducer((attempt: number) => attempt + 1, 0);

  return (
    <main>
      <ApplicationConnection key={connectionAttempt} />
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
          <SettingsWindow />
          <StatusDetails />
        </>
      )}
    </main>
  );
}
