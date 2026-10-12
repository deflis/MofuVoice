import { useVoiceSettings } from "../settingsHooks";
import { SemitonesInput } from "./SemitonesInput";

export function VoiceSettings() {
  const { pitch, formantShift, preserveFormants, update } = useVoiceSettings();
  return (
    <section>
      <h2>声の調整</h2>
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
