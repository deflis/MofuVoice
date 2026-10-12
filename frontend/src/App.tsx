import { useEffect } from "react";
import { useSettingsStore } from "./settingsStore";
import { useSettingsReady } from "./settingsHooks";
import { AudioControls } from "./components/AudioControls";
import { SettingsPanel } from "./components/SettingsPanel";
import { SaveSettingsButton } from "./components/SaveSettingsButton";
import { AppHeader, Notifications, StatusDetails } from "./components/Status";

export function App() {
  const ready = useSettingsReady();
  useEffect(() => useSettingsStore.getState().connect(), []);

  return (
    <main>
      <AppHeader />
      <Notifications />
      {!ready ? (
        <p>設定を読み込んでいます…</p>
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
