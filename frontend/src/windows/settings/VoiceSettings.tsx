import { DetailsPopup } from "./DetailsPopup";
import { useVoiceSettings } from "./hooks";
import { SemitonesInput } from "./SemitonesInput";

export function VoiceSettings() {
  const { pitch, formantShift, preserveFormants, update } = useVoiceSettings();
  return (
    <section>
      <h2>声の変換</h2>
      <SemitonesInput
        label="声の高さ（ピッチ）"
        value={pitch}
        onChange={(value) => update("pitch", value)}
      />
      <fieldset className="settings" disabled={!preserveFormants}>
        <SemitonesInput
          label="声の響き（フォルマント）"
          lowerLabel="太く"
          upperLabel="明るく"
          value={formantShift}
          onChange={(value) => update("formantShift", value)}
        />
      </fieldset>
      <button
        type="button"
        onClick={() => {
          update("pitch", "0");
          update("formantShift", "0");
        }}
      >
        元の声に戻す
      </button>
      <DetailsPopup title="声の変換">
        <SemitonesInput
          precise
          label="声の高さ（ピッチ）"
          value={pitch}
          onChange={(value) => update("pitch", value)}
        />
        <fieldset className="settings" disabled={!preserveFormants}>
          <SemitonesInput
            precise
            label="声の響き（フォルマント）"
            value={formantShift}
            onChange={(value) => update("formantShift", value)}
          />
        </fieldset>
        <label className="check">
          <input
            type="checkbox"
            checked={preserveFormants}
            onChange={(event) => {
              update("preserveFormants", event.target.checked);
              if (!event.target.checked) {
                update("formantShift", "0");
              }
            }}
          />
          声の響きを独立して調整する
        </label>
        <p>オフでは高さの変更に合わせて響きも変わります。響きの調整値は0に戻ります。</p>
      </DetailsPopup>
    </section>
  );
}
