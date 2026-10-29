import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.noise);
  const setEnabled = useSettingsStore((state) => state.noise.setEnabled);
  const valid = v.safeParse(noiseSchema, settings).success;
  const total = settings.attackMs + settings.releaseMs;
  const attackX = 68 + (settings.attackMs / total) * 150;
  return (
    <Card
      id="noiseGate"
      title={t("ノイズゲート")}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {valid ? (
        <>
          <Plot
            description={t(
              "ゲートが開いてから閉じるまでの時間応答の模式図。実際の検出レベルやゲインではありません。",
            )}
            xLabel={t("時間（模式図）")}
            yLabel={t("開閉")}
          >
            <path
              className={styles["filter-curve"]}
              d={`M36 86H68L${attackX} 24H${attackX + 50}L268 86H308`}
            />
            <text x="80" y="70">
              {t("開く")}
            </text>
            <text x="240" y="70">
              {t("閉じる")}
            </text>
          </Plot>
          <Value>
            {settings.thresholdMode === "auto"
              ? t("しきい値: 自動推定（実測値は表示しません）")
              : t("しきい値: {0} dBFS", { v0: num(settings.thresholdDb) })}
          </Value>
          <Value>
            {t("アタック")}
            {num(settings.attackMs)}
            {t("ms · リリース")}
            {num(settings.releaseMs)} ms
          </Value>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function DeesserCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.deesser);
  const setEnabled = useSettingsStore((state) => state.deesser.setEnabled);
  const valid = v.safeParse(deesserSchema, settings).success;
  const frequencyX = 36 + (Math.log10(settings.frequencyHz / 1000) / Math.log10(20)) * 272;
  return (
    <Card
      id="deesser"
      title={t("ディエッサー")}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {valid ? (
        <>
          <Plot
            description={t(
              "歯擦音の検出帯域と最大抑制量の模式図。固定EQの周波数応答ではありません。",
            )}
            xLabel={t("1 → 20 kHz（対数・模式図）")}
            yLabel={t("検出帯域")}
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
              {t("最大")}
              {num(settings.rangeDb)}
              {t("dB 抑制")}
            </text>
          </Plot>
          <Value>
            {t("検出")}
            {num(settings.frequencyHz)}
            {t("Hz〜 · しきい値")}
            {num(settings.thresholdDb)} dBFS
          </Value>
          <Value>
            {t("アタック")}
            {num(settings.attackMs)}
            {t("ms · リリース")}
            {num(settings.releaseMs)} ms
          </Value>
          <small>{t("声変換の前に歯擦音を抑えます。図は検出設定の目安です。")}</small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

const x = (shift: number) => 36 + ((shift + 12) / 24) * 272;

export function VoiceCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.voice);
  const setEnabled = useSettingsStore((state) => state.voice.setEnabled);
  const valid = v.safeParse(voiceSchema, settings).success;
  return (
    <Card
      id="voice"
      title={t("ピッチ／フォルマント変換")}
      onSelect={onSelect}
      locked={locked}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
    >
      {valid ? (
        <>
          <Plot
            description={t(
              "ピッチと独立フォルマント調整の相対移動量。入力音声の基本周波数ではありません。",
            )}
            xLabel={t("−12 → +12 半音（相対設定）")}
            yLabel={t("移動量")}
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
              {t("ピッチ")}
            </text>
            <text x="40" y="59">
              {t("フォルマント")}
            </text>
          </Plot>
          <Value>
            {t("ピッチ")}
            {num(settings.pitch)}
            {t("半音（")}
            {num(100 * 2 ** (settings.pitch / 12))}
            {t("%） · フォルマント")}
            {num(settings.formantShift)}
            {t("半音")}
          </Value>
          <Value>
            {t("方式")}
            {settings.pitchAlgorithm}
            {t("· フォルマント独立調整")} {settings.preserveFormants ? t("有効") : t("無効")}
          </Value>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function LevelCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
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
      title={t("後処理の音量調整・コンプレッサー")}
      onSelect={onSelect}
      locked={locked}
      toggles={[
        { label: t("自動音量調整"), enabled: settings.enabled, onChange: setAutoLevelEnabled },
        {
          label: t("コンプレッサー"),
          enabled: settings.compressorEnabled,
          onChange: setCompressorEnabled,
        },
      ]}
    >
      {valid ? (
        <>
          {settings.enabled && (
            <Value>
              {t("目標")}
              {num(settings.targetDb)}
              {t("dBFS · 増幅上限")}
              {num(settings.maxGainDb)}
              {t("dB · 応答")} {num(settings.responseMs)} ms
            </Value>
          )}
          {settings.compressorEnabled && (
            <>
              <Plot
                description={t(
                  "コンプレッサー単体の設定静特性。6 dBソフトニーを含みます。時間応答と内部ピークガードの効果は含みません。",
                )}
                xLabel={t("入力 −60 → 0 dBFS")}
                yLabel={t("出力 dBFS")}
              >
                <path className={styles["filter-reference"]} d="M36 86L308 22" />
                <path className={styles["filter-curve"]} d={curvePath(points)} />
              </Plot>
              <Value>
                {t("圧縮しきい値")}
                {num(settings.thresholdDb)}
                {t("dBFS · 比率")}
                {num(settings.ratio)}
                {t(":1 · アタック")}
                {num(settings.attackMs)}
                {t("ms · リリース")}
                {num(settings.releaseMs)} ms
              </Value>
              <small>
                {t(
                  "図は圧縮の設定静特性です。有効な音量処理が動作する場合、内部ピークガード（−1 dBFS）を適用します。",
                )}
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
