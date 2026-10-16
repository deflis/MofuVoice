import { useVoiceSettings } from "./hooks";
import { SemitonesInput } from "./SemitonesInput";
import * as v from "valibot";
import { voiceSchema } from "./schema";

export function VoiceSettings() {
  const {
    pitchAlgorithm,
    setPitchAlgorithm,
    pitch,
    formantShift,
    preserveFormants,
    setPitch,
    setFormantShift,
    setPreserveFormants,
  } = useVoiceSettings();
  const tdPsola = pitchAlgorithm === "td-psola";
  return (
    <section>
      <h2>声の変換</h2>
      <label>
        ピッチ変換方式
        <select
          value={pitchAlgorithm}
          onChange={(event) =>
            setPitchAlgorithm(v.parse(voiceSchema.entries.pitchAlgorithm, event.target.value))
          }
        >
          <option value="phase-locked">フェーズロック（標準）</option>
          <option value="pvsola">PVSOLAアプローチ（実験）</option>
          <option value="td-psola">TD-PSOLA（実験）</option>
        </select>
      </label>
      <p>同じピッチ・響きの設定で聞き比べてください。変更は保存後、次の音声開始時に反映します。</p>
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
          disabled={tdPsola}
          onChange={(event) => {
            setPreserveFormants(event.target.checked);
            if (!event.target.checked) {
              setFormantShift(0);
            }
          }}
        />
        声の響きを独立して調整する
      </label>
      {tdPsola ? (
        <p>
          TD-PSOLAで高さを変え、響きの移動にはスペクトル処理を使います。DSP遅延は48 kHzで96
          ms、響きの移動が0以外なら約106.7 msです。
        </p>
      ) : (
        <p>オフでは高さの変更に合わせて響きも変わります。響きの調整値は0に戻ります。</p>
      )}
    </section>
  );
}
