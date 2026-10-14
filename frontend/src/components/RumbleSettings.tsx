import { useVoiceSettings } from "../settingsHooks";

export function RumbleSettings() {
  const { rumbleFilter, update } = useVoiceSettings();
  return (
    <section>
      <h2>低い唸りを減らす</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={rumbleFilter}
          onChange={(event) => update("rumbleFilter", event.target.checked)}
        />
        有効
      </label>
      <p>机の振動や空調の低い音が気になるときに使います。</p>
    </section>
  );
}
