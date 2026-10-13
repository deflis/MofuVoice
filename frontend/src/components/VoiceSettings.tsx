import { useVoiceSettings } from "../settingsHooks";
import { SemitonesInput } from "./SemitonesInput";

export function VoiceSettings() {
  const { rumbleFilter, pitch, formantShift, preserveFormants, update } = useVoiceSettings();
  return (
    <section>
      <h2>声の調整</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={rumbleFilter}
          onChange={(event) => update("rumbleFilter", event.target.checked)}
        />
        低域ノイズ除去（80 Hz）
      </label>
      <p>机の振動や空調の低い唸りを抑えます。低い声が薄くなる場合はオフにしてください。</p>
      <SemitonesInput label="Pitch" value={pitch} onChange={(value) => update("pitch", value)} />
      <SemitonesInput
        label="Formant shift"
        value={formantShift}
        onChange={(value) => update("formantShift", value)}
      />
      <label className="check">
        <input
          type="checkbox"
          checked={preserveFormants}
          onChange={(event) => update("preserveFormants", event.target.checked)}
        />
        Preserve formants
      </label>
    </section>
  );
}
