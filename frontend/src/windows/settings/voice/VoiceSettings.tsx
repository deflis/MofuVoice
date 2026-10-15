import { useVoiceSettings } from "./hooks";
import { SemitonesInput } from "./SemitonesInput";

export function VoiceSettings() {
  const { pitch, formantShift, preserveFormants, setPitch, setFormantShift, setPreserveFormants } =
    useVoiceSettings();
  return (
    <section>
      <h2>声の変換</h2>
      <SemitonesInput
        lowerLabel="低く"
        upperLabel="高く"
        label="声の高さ（ピッチ）"
        value={pitch}
        onChange={(value) => setPitch(value)}
      />
      <fieldset className="settings" disabled={!preserveFormants}>
        <SemitonesInput
          label="声の響き（フォルマント）"
          lowerLabel="太く"
          upperLabel="明るく"
          value={formantShift}
          onChange={(value) => setFormantShift(value)}
        />
      </fieldset>
      <button
        type="button"
        onClick={() => {
          setPitch(0);
          setFormantShift(0);
        }}
      >
        元の声に戻す
      </button>
      <label className="check">
        <input
          type="checkbox"
          checked={preserveFormants}
          onChange={(event) => {
            setPreserveFormants(event.target.checked);
            if (!event.target.checked) {
              setFormantShift(0);
            }
          }}
        />
        声の響きを独立して調整する
      </label>
      <p>オフでは高さの変更に合わせて響きも変わります。響きの調整値は0に戻ります。</p>
    </section>
  );
}
