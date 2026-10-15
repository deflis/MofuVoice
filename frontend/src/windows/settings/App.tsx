import { useEffect } from "react";
import { useSettingsStore } from "./store";
import { Notifications } from "./Notifications";
import { SettingsWindow } from "./SettingsWindow";

export function SettingsApp() {
  const status = useSettingsStore((state) => state.status);
  const retry = useSettingsStore((state) => state.retry);
  useEffect(() => useSettingsStore.getState().mount(), []);

  let body;
  if (status === "loading") {
    body = (
      <div>
        <p>設定を読み込んでいます…</p>
      </div>
    );
  } else if (status === "loadFailed") {
    body = (
      <div>
        <button type="button" onClick={() => retry()}>
          設定の読み込みを再試行
        </button>
      </div>
    );
  } else {
    body = <SettingsWindow />;
  }

  return (
    <main>
      <Notifications />
      {body}
    </main>
  );
}
