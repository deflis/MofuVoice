import { DetailsPopup } from "../DetailsPopup";
import { useSettingsStore } from "../store";
import { useVoiceSettings } from "./hooks";
import { SemitonesInput } from "./SemitonesInput";
import * as v from "valibot";
import { voiceSchema } from "./schema";

export function VoiceSettings() {
  const {
    analysisWindow,
    synthesisWindow,
    setAnalysisWindow,
    setSynthesisWindow,
    pitchAlgorithm,
    setPitchAlgorithm,
    pitch,
    formantShift,
    preserveFormants,
    setPitch,
    setFormantShift,
    setPreserveFormants,
  } = useVoiceSettings();
  const sampleRate = useSettingsStore((state) => state.devices.sampleRate);
  const spectral = pitchAlgorithm === "phase-locked" || pitchAlgorithm === "pvsola-sync";
  const tdPsola = pitchAlgorithm === "td-psola" || pitchAlgorithm === "td-psola-resampled";
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
          <option value="pvsola">PVSOLA 論文方式（実験）</option>
          <option value="pvsola-sync">スペクトル位相同期（従来方式）</option>
          <option value="td-psola">TD-PSOLA（実験）</option>
          <option value="td-psola-resampled">TD-PSOLA＋時間伸縮リサンプリング（実験）</option>
          <option value="fd-psola">FD-PSOLA（実験）</option>
        </select>
      </label>
      <p>同じピッチ・響きの設定で聞き比べてください。変更は保存後、次の音声開始時に反映します。</p>
      <DetailsPopup title="声の変換" buttonLabel="詳細設定">
        <fieldset className="settings" disabled={!spectral}>
          <label>
            解析ウィンドウ長
            <select
              value={analysisWindow}
              onChange={(event) =>
                setAnalysisWindow(
                  v.parse(voiceSchema.entries.analysisWindow, Number(event.target.value)),
                )
              }
            >
              {[1024, 2048, 4096, 8192].map((size) => (
                <option key={size} value={size}>
                  {size}サンプル（{((size / sampleRate) * 1000).toFixed(1)} ms）
                </option>
              ))}
            </select>
          </label>
          <label>
            合成ウィンドウ長
            <select
              value={synthesisWindow}
              onChange={(event) =>
                setSynthesisWindow(
                  v.parse(voiceSchema.entries.synthesisWindow, Number(event.target.value)),
                )
              }
            >
              {[256, 512, 1024, 2048]
                .filter((size) => size <= analysisWindow)
                .map((size) => (
                  <option key={size} value={size}>
                    {size}サンプル（{((size / sampleRate) * 1000).toFixed(1)} ms）
                  </option>
                ))}
            </select>
          </label>
          <p>
            解析ウィンドウ長を長くすると周波数を細かく分析できます。合成ウィンドウ長を短くすると遅延を減らせます。音質は声や設定によって変わるため、聞き比べて調整してください。
          </p>
          <p>
            変換時のDSP遅延：{((synthesisWindow / sampleRate) * 1000).toFixed(1)}{" "}
            ms。高さと響きがともに0の場合は0 msです。入出力バッファーと機器の遅延は含みません。
          </p>
          <button
            type="button"
            onClick={() => {
              setAnalysisWindow(2048);
              setSynthesisWindow(512);
            }}
          >
            ウィンドウ長を標準に戻す
          </button>
        </fieldset>
        <p>
          フェーズロックとスペクトル位相同期に適用します。変更は保存後、次の音声開始時に反映します。
        </p>
      </DetailsPopup>
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
      <p>{algorithmDescription(pitchAlgorithm)}</p>
    </section>
  );
}

function algorithmDescription(algorithm: string): string {
  switch (algorithm) {
    case "td-psola":
      return "TD-PSOLAで高さを変え、響きの移動にはスペクトル処理を使います。DSP遅延は48 kHzで96 ms、響きの移動時に約106.7 msです。";
    case "td-psola-resampled":
      return "TD-PSOLAの時間伸縮と帯域制限リサンプリングで高さと響きを独立して変えます。DSP遅延は48 kHzで96 msです。";
    case "pvsola":
      return "PVSOLA論文方式は位相ボコーダーで時間伸縮し、相互相関で整列した入力フレームを周期的に挿入してから再生速度を変えます。DSP遅延は48 kHzで約96 msです。響きの保持と独立調整に対応します。";
    case "pvsola-sync":
      return "スペクトル位相同期は既存の直接スペクトル変換方式です。主要な周波数成分の相関を使って位相を補正します。DSP遅延は詳細設定の合成ウィンドウ長で変わります。";
    case "fd-psola":
      return "FD-PSOLAはピッチ同期フレームのスペクトルを変換します。フォルマント保持を有効にすると響きの包絡を維持します。DSP遅延は48 kHzで96 ms、響きの移動時に約106.7 msです。";
    default:
      return "オフでは高さの変更に合わせて響きも変わります。響きの調整値は0に戻ります。";
  }
}
