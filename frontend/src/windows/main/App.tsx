import { useEffect, useReducer } from "react";
import { connectMain } from "./connect";
import { useSettingsReady, useSettingsLoadFailed } from "../../settingsHooks";
import { Notifications } from "../../components/Notifications";
import { Home } from "./Home";

function MainConnection() {
  useEffect(() => connectMain().disconnect, []);
  return null;
}

export function MainApp() {
  const ready = useSettingsReady();
  const loadFailed = useSettingsLoadFailed();
  const [connectionAttempt, retryConnection] = useReducer((attempt: number) => attempt + 1, 0);

  return (
    <main>
      <MainConnection key={connectionAttempt} />
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
        <Home />
      )}
    </main>
  );
}
