import { useSaveSettings } from "./hooks";

export function SaveSettingsButton() {
  const { disabled, save } = useSaveSettings();
  return (
    <footer>
      <button type="button" disabled={disabled} onClick={() => void save()}>
        設定を保存
      </button>
    </footer>
  );
}
