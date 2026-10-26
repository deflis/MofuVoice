import styles from "./overview.module.css";
import * as v from "valibot";
import { useSettingsStore } from "../store";
import { noiseSchema } from "../noise/schema";
import { deesserSchema } from "../deesser/schema";
import { voiceSchema } from "../voice/schema";
import { levelSchema } from "../level/schema";
import { Card, InvalidGraph, Value, num, type CardProps } from "./Card";
import { Plot, curvePath } from "./Plot";
import { compressorOutputDb } from "./transfer";

export function NoiseGateCard({ onSelect, locked }: CardProps) {
  const settings = useSettingsStore((state) => state.noise);
  const setEnabled = useSettingsStore((state) => state.noise.setEnabled);
  const valid = v.safeParse(noiseSchema, settings).success;
  const total = settings.attackMs + settings.releaseMs;
  const attackX = 68 + (settings.attackMs / total) * 150;
  return (
    <Card
      id="noiseGate"
      title="ノイズゲート"
      toggles={[{ label: "有効", enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {valid ? (
        <>
          <Plot
            description="ゲートが開いてから閉じるまでの時間応答の模式図。実際の検出レベルやゲインではありません。"
            xLabel="時間（模式図）"
            yLabel="開閉"
          >
            <path
              className={styles["filter-curve"]}
              d={`M36 86H68L${attackX} 24H${attackX + 50}L268 86H308`}
            />
            <text x="80" y="70">
              開く
            </text>
            <text x="240" y="70">
              閉じる
            </text>
          </Plot>
          <Value>
            {settings.thresholdMode === "auto"
              ? "しきい値: 自動推定（実測値は表示しません）"
              : `しきい値: ${num(settings.thresholdDb)} dBFS`}
          </Value>
          <Value>
            アタック {num(settings.attackMs)} ms · リリース {num(settings.releaseMs)} ms
          </Value>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function DeesserCard({ onSelect, locked }: CardProps) {
  const settings = useSettingsStore((state) => state.deesser);
  const setEnabled = useSettingsStore((state) => state.deesser.setEnabled);
  const valid = v.safeParse(deesserSchema, settings).success;
  const frequencyX = 36 + (Math.log10(settings.frequencyHz / 1000) / Math.log10(20)) * 272;
  return (
    <Card
      id="deesser"
      title="ディエッサー"
      toggles={[{ label: "有効", enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {valid ? (
        <>
          <Plot
            description="歯擦音の検出帯域と最大抑制量の模式図。固定EQの周波数応答ではありません。"
            xLabel="1 → 20 kHz（対数・模式図）"
            yLabel="検出帯域"
          >
            <rect
              className={styles["filter-area"]}
              x={frequencyX}
              y="22"
              width={308 - frequencyX}
              height="64"
            />
            <path className={styles["filter-curve"]} d={`M${frequencyX} 22V86`} />
            <text x="45" y="40">
              最大 {num(settings.rangeDb)} dB 抑制
            </text>
          </Plot>
          <Value>
            検出 {num(settings.frequencyHz)} Hz〜 · しきい値 {num(settings.thresholdDb)} dBFS
          </Value>
          <Value>
            アタック {num(settings.attackMs)} ms · リリース {num(settings.releaseMs)} ms
          </Value>
          <small>声変換の前に歯擦音を抑えます。図は検出設定の目安です。</small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

const x = (shift: number) => 36 + ((shift + 12) / 24) * 272;

export function VoiceCard({ onSelect, locked }: CardProps) {
  const settings = useSettingsStore((state) => state.voice);
  const setEnabled = useSettingsStore((state) => state.voice.setEnabled);
  const valid = v.safeParse(voiceSchema, settings).success;
  return (
    <Card
      id="voice"
      title="ピッチ／フォルマント変換"
      onSelect={onSelect}
      locked={locked}
      toggles={[{ label: "有効", enabled: settings.enabled, onChange: setEnabled }]}
    >
      {valid ? (
        <>
          <Plot
            description="ピッチと独立フォルマント調整の相対移動量。入力音声の基本周波数ではありません。"
            xLabel="−12 → +12 半音（相対設定）"
            yLabel="移動量"
          >
            <path className={styles["filter-reference"]} d="M172 16V86" />
            <path
              className={styles["filter-curve"]}
              d={`M172 38H${x(settings.pitch)}M172 68H${x(settings.formantShift)}`}
            />
            <circle className={styles["filter-point"]} cx={x(settings.pitch)} cy="38" r="4" />
            <circle
              className={styles["filter-point"]}
              cx={x(settings.formantShift)}
              cy="68"
              r="4"
            />
            <text x="40" y="29">
              ピッチ
            </text>
            <text x="40" y="59">
              フォルマント
            </text>
          </Plot>
          <Value>
            ピッチ {num(settings.pitch)} 半音（{num(100 * 2 ** (settings.pitch / 12))}%） ·
            フォルマント {num(settings.formantShift)} 半音
          </Value>
          <Value>
            方式 {settings.pitchAlgorithm} · フォルマント独立調整{" "}
            {settings.preserveFormants ? "有効" : "無効"}
          </Value>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function LevelCard({ onSelect, locked }: CardProps) {
  const settings = useSettingsStore((state) => state.level);
  const setAutoLevelEnabled = useSettingsStore((state) => state.level.setEnabled);
  const setCompressorEnabled = useSettingsStore((state) => state.level.setCompressorEnabled);
  const valid = v.safeParse(levelSchema, settings).success;
  const points: [number, number][] =
    valid && settings.compressorEnabled
      ? Array.from({ length: 121 }, (_, i) => {
          const input = -60 + i / 2;
          return [
            36 + (i / 120) * 272,
            86 - ((compressorOutputDb(input, settings.thresholdDb, settings.ratio) + 60) / 60) * 64,
          ];
        })
      : [];
  return (
    <Card
      id="level"
      title="後処理の音量調整・コンプレッサー"
      onSelect={onSelect}
      locked={locked}
      toggles={[
        { label: "自動音量調整", enabled: settings.enabled, onChange: setAutoLevelEnabled },
        {
          label: "コンプレッサー",
          enabled: settings.compressorEnabled,
          onChange: setCompressorEnabled,
        },
      ]}
    >
      {valid ? (
        <>
          {settings.enabled && (
            <Value>
              目標 {num(settings.targetDb)} dBFS · 増幅上限 {num(settings.maxGainDb)} dB · 応答{" "}
              {num(settings.responseMs)} ms
            </Value>
          )}
          {settings.compressorEnabled && (
            <>
              <Plot
                description="コンプレッサー単体の設定静特性。6 dBソフトニーを含みます。時間応答と内部ピークガードの効果は含みません。"
                xLabel="入力 −60 → 0 dBFS"
                yLabel="出力 dBFS"
              >
                <path className={styles["filter-reference"]} d="M36 86L308 22" />
                <path className={styles["filter-curve"]} d={curvePath(points)} />
              </Plot>
              <Value>
                圧縮しきい値 {num(settings.thresholdDb)} dBFS · 比率 {num(settings.ratio)}:1 ·
                アタック {num(settings.attackMs)} ms · リリース {num(settings.releaseMs)} ms
              </Value>
              <small>
                図は圧縮の設定静特性です。有効な音量処理が動作する場合、内部ピークガード（−1
                dBFS）を適用します。
              </small>
            </>
          )}
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}
