import styles from "../settings.module.css";
import { pitchLatency } from "./latency";
import { DetailsPopup } from "../DetailsPopup";
import { useSettingsStore } from "../store";
import { useVoiceSettings } from "./hooks";
import { SemitonesInput } from "./SemitonesInput";
import { useId } from "react";
import * as v from "valibot";
import { voiceSchema, type VoiceValues } from "./schema";

export function VoiceSettings() {
  const {
    enabled,
    setEnabled,
    pitchUnit,
    setPitchUnit,
    psolaPitchEstimator,
    setPsolaPitchEstimator,
    psolaAnalysisWindow,
    psolaSynthesisWindow,
    setPsolaAnalysisWindow,
    setPsolaSynthesisWindow,
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
  const delay = pitchLatency(
    pitchAlgorithm,
    synthesisWindow,
    psolaAnalysisWindow,
    psolaSynthesisWindow,
    formantShift,
  );
  const psola = pitchAlgorithm === "fd-psola" || pitchAlgorithm.startsWith("td-psola");
  const vocoder =
    pitchAlgorithm === "phase-locked" ||
    pitchAlgorithm === "pvsola-sync" ||
    ((pitchAlgorithm === "td-psola" || pitchAlgorithm === "fd-psola") && formantShift !== 0);
  const tdPsola = pitchAlgorithm === "td-psola" || pitchAlgorithm === "td-psola-resampled";
  const pitchAlgorithmId = useId();
  const pitchEstimatorId = useId();
  return (
    <section>
      <h2>声の変換</h2>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => setEnabled(event.target.checked)}
        />
        有効
      </label>
      {!enabled && <p>バイパス中。変換量は保持されます。</p>}
      <fieldset className={styles.settings} disabled={!enabled}>
        <DetailsPopup title="声の変換" buttonLabel="詳細設定">
          <label>
            <span id={pitchAlgorithmId}>ピッチ変換方式</span>
            <select
              aria-labelledby={pitchAlgorithmId}
              value={pitchAlgorithm}
              onChange={(event) =>
                setPitchAlgorithm(v.parse(voiceSchema.entries.pitchAlgorithm, event.target.value))
              }
            >
              <optgroup label="フェーズボコーダー">
                <option value="phase-locked">位相ロック（標準）</option>
                <option value="pvsola-sync">スペクトル位相同期</option>
              </optgroup>
              <optgroup label="PVSOLA">
                <option value="pvsola">PVSOLA</option>
              </optgroup>
              <optgroup label="PSOLA">
                <option value="td-psola">TD-PSOLA＋フェーズボコーダー</option>
                <option value="td-psola-resampled">TD-PSOLA＋リサンプリング</option>
                <option value="fd-psola">FD-PSOLA</option>
              </optgroup>
            </select>
          </label>
          <p>
            同じピッチとフォルマントの設定で聞き比べてください。変更は保存後、次の音声開始時に反映します。
          </p>
          <p>{algorithmDescription(pitchAlgorithm)}</p>
          <WindowControls
            title="ボコーダーのウィンドウ"
            analysis={analysisWindow}
            synthesis={synthesisWindow}
            setAnalysis={setAnalysisWindow}
            setSynthesis={setSynthesisWindow}
            disabled={!vocoder}
          />
          <p>
            位相ロック（標準）・スペクトル位相同期と、TD-PSOLA＋フェーズボコーダー／FD-PSOLAのフォルマント補正に使います。
          </p>
          <label>
            <span id={pitchEstimatorId}>PSOLAのピッチ推定</span>
            <select
              aria-labelledby={pitchEstimatorId}
              value={psolaPitchEstimator}
              disabled={!psola}
              onChange={(event) =>
                setPsolaPitchEstimator(
                  v.parse(voiceSchema.entries.psolaPitchEstimator, event.target.value),
                )
              }
            >
              <optgroup label="YIN">
                <option value="yin">YIN（標準）</option>
              </optgroup>
              <optgroup label="MPM">
                <option value="mpm">MPM</option>
                <option value="mpm-fft">MPM＋FFT</option>
              </optgroup>
              <optgroup label="NCCF">
                <option value="nccf">NCCF</option>
              </optgroup>
            </select>
          </label>
          <p>
            声によって推定精度が変わります。低音や息の多い声で聞き比べてください。追加の解析待ちはありません。MPMは正規化相関、MPM＋FFTはFFT自己相関で、ウィンドウ長によって計算負荷が変わります。NCCFは過去の周期も参考にします。
          </p>
          <WindowControls
            title="PSOLAのウィンドウ"
            analysis={psolaAnalysisWindow}
            synthesis={psolaSynthesisWindow}
            setAnalysis={setPsolaAnalysisWindow}
            setSynthesis={setPsolaSynthesisWindow}
            disabled={!psola}
          />
          <p>
            TD-PSOLA＋フェーズボコーダーとTD-PSOLA＋リサンプリングはピッチ周期（波形単位）を基準にウィンドウを適用し、FD-PSOLAは解析ウィンドウと合成ウィンドウのサイズを設定します。設定値によって遅延時間や音質が変化します。
          </p>
          <p>
            変換時のDSP遅延の合計：{((delay / sampleRate) * 1000).toFixed(1)}{" "}
            ms。高さとフォルマントがともに0の場合は0
            msです。入出力バッファーと機器の遅延は含みません。
          </p>
          <p>変更は保存後、次の音声開始時に反映します。PVSOLAは固定長です。</p>
        </DetailsPopup>
        <label>
          ピッチの表示単位
          <select
            value={pitchUnit}
            onChange={(event) =>
              setPitchUnit(v.parse(voiceSchema.entries.pitchUnit, event.target.value))
            }
          >
            <option value="semitones">半音</option>
            <option value="percent">パーセント</option>
          </select>
        </label>
        {pitchUnit === "percent" && (
          <p>100%が元の高さ、50%で1オクターブ下、200%で1オクターブ上です。</p>
        )}
        <SemitonesInput
          percent={pitchUnit === "percent"}
          lowerLabel="低く"
          upperLabel="高く"
          label="声の高さ（ピッチ）"
          value={pitch}
          onChange={(value) => setPitch(value)}
        />
        <fieldset className={styles.settings} disabled={!preserveFormants}>
          <SemitonesInput
            label="フォルマント"
            lowerLabel="太く"
            upperLabel="明るく"
            value={formantShift}
            onChange={(value) => setFormantShift(value)}
          />
        </fieldset>
        <p>声の太さ・明るさです。残響（リバーブ）とは別の調整です。</p>
        <button
          type="button"
          onClick={() => {
            setPitch(0);
            setFormantShift(0);
          }}
        >
          元の声に戻す
        </button>
        <label className={styles.check}>
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
          フォルマントを独立して調整する
        </label>
      </fieldset>
    </section>
  );
}

function algorithmDescription(algorithm: string): string {
  switch (algorithm) {
    case "td-psola":
      return "TD-PSOLA＋フェーズボコーダーは、TD-PSOLAで高さを変え、フォルマントの移動にはフェーズボコーダーを使います。DSP遅延はウィンドウ設定とフォルマント移動の有無で変わります。";
    case "td-psola-resampled":
      return "TD-PSOLA＋リサンプリングは、時間伸縮と帯域制限リサンプリングで高さとフォルマントを独立して変えます。DSP遅延はウィンドウ設定で変わります。";
    case "pvsola":
      return "PVSOLAは位相ボコーダーで時間伸縮し、相互相関で整列した入力フレームを周期的に挿入してから再生速度を変えます。DSP遅延は48 kHzで約96 msです。フォルマントの保持と独立調整に対応します。";
    case "pvsola-sync":
      return "スペクトル位相同期は、直接スペクトル変換で主要な周波数成分の相関を使って位相を補正します。DSP遅延は詳細設定の合成ウィンドウ長で変わります。";
    case "fd-psola":
      return "FD-PSOLAはピッチ同期フレームのスペクトルを変換します。「フォルマントを独立して調整する」をオンにすると、フォルマントの包絡を維持します。DSP遅延はウィンドウ設定とフォルマント移動の有無で変わります。";
    default:
      return "位相ロック（標準）は周波数成分の位相関係を保ちながら高さを変えます。DSP遅延は合成ウィンドウ長で変わります。";
  }
}

function WindowControls({
  title,
  analysis,
  synthesis,
  setAnalysis,
  setSynthesis,
  disabled,
}: {
  title: string;
  analysis: VoiceValues["analysisWindow"];
  synthesis: VoiceValues["synthesisWindow"];
  setAnalysis: (value: VoiceValues["analysisWindow"]) => void;
  setSynthesis: (value: VoiceValues["synthesisWindow"]) => void;
  disabled: boolean;
}) {
  const sampleRate = useSettingsStore((state) => state.devices.sampleRate);
  return (
    <fieldset className={styles.settings} disabled={disabled}>
      <legend>{title}</legend>
      <label>
        解析ウィンドウ長
        <select
          value={analysis}
          onChange={(event) =>
            setAnalysis(v.parse(voiceSchema.entries.analysisWindow, Number(event.target.value)))
          }
        >
          {([1024, 2048, 4096, 8192] as const).map((size) => (
            <option key={size} value={size}>
              {size}サンプル（{((size / sampleRate) * 1000).toFixed(1)} ms）
            </option>
          ))}
        </select>
      </label>
      <label>
        合成ウィンドウ長
        <select
          value={synthesis}
          onChange={(event) =>
            setSynthesis(v.parse(voiceSchema.entries.synthesisWindow, Number(event.target.value)))
          }
        >
          {([256, 512, 1024, 2048] as const)
            .filter((size) => size <= analysis)
            .map((size) => (
              <option key={size} value={size}>
                {size}サンプル（{((size / sampleRate) * 1000).toFixed(1)} ms）
              </option>
            ))}
        </select>
      </label>
      <button
        type="button"
        onClick={() => {
          setAnalysis(2048);
          setSynthesis(512);
        }}
      >
        ウィンドウ長を既定値に戻す
      </button>
    </fieldset>
  );
}
