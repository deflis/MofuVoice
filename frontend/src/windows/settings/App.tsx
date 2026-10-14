import { useEffect, useReducer } from "react";
import { connectSettings } from "./connect";
import { useSettingsReady, useSettingsLoadFailed } from "../../settingsHooks";
import { Notifications } from "../../components/Notifications";
import { SettingsWindow } from "../../components/SettingsWindow";

function SettingsConnection() {
  useEffect(() => connectSettings().disconnect, []);
  return null;
}

export function SettingsApp() {
  const ready = useSettingsReady();
  const loadFailed = useSettingsLoadFailed();
  const [connectionAttempt, retryConnection] = useReducer((attempt: number) => attempt + 1, 0);

  return (
    <main>
      <SettingsConnection key={connectionAttempt} />
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
        <SettingsWindow />
      )}
    </main>
  );
}
