import { SettingsPanel } from "./SettingsPanel";
import { SaveSettingsButton } from "./SaveSettingsButton";
import { useAudioStore } from "../audioStore";

export function SettingsWindow() {
  const busy = useAudioStore((state) => state.snapshot.busy || state.pending);
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
