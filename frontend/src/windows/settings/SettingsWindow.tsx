import { SettingsPanel } from "./SettingsPanel";
import { SaveSettingsButton } from "./SaveSettingsButton";
import { useSettingsStore } from "./store";

export function SettingsWindow() {
  const busy = useSettingsStore(
    (state) => state.audioBusy || state.status === "saving" || state.status === "switchingBackend",
  );
  return (
    <div className="settings-window">
      <header>
        <h1>設定</h1>
        <span className="muted">Voice Changer</span>
      </header>
      {busy && <output>音声を停止すると設定を変更できます。</output>}
      <SettingsPanel />
      <SaveSettingsButton />
    </div>
  );
}
