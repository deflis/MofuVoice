import { useTranslation } from "react-i18next";
import styles from "./overview.module.css";
import * as v from "valibot";
import { useSettingsStore } from "../store";
import { limiterSchema } from "../limiter/schema";
import { dereverbSchema } from "../dereverb/schema";
import { reverbSchema } from "../reverb/schema";
import { compressorSchema } from "../compressor/schema";
import { Card, InvalidGraph, Value, num, type CardProps } from "./Card";
import { Plot, TransferPlot, curvePath } from "./Plot";

export function CompressorCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.compressor);
  const setEnabled = useSettingsStore((state) => state.compressor.setEnabled);
  return (
    <Card
      id="compressor"
      title={t("変換前コンプレッサー")}
      onSelect={onSelect}
      locked={locked}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
    >
      {v.safeParse(compressorSchema, settings).success ? (
        <>
          <TransferPlot
            threshold={settings.thresholdDb}
            ratio={settings.ratio}
            description={t(
              "変換前コンプレッサーのハードニー設定静特性。検出と時間応答を含みません。",
            )}
          />
          <Value>
            {t("しきい値")}
            {num(settings.thresholdDb)}
            {t("dBFS · 比率")}
            {num(settings.ratio)}:1
          </Value>
          <Value>
            {t("アタック")}
            {num(settings.attackMs)}
            {t("ms · リリース")}
            {num(settings.releaseMs)} ms
          </Value>
          <small>{t("図は設定静特性です。実測の抑制量ではありません。")}</small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function LimiterCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.limiter);
  const setEnabled = useSettingsStore((state) => state.limiter.setEnabled);
  return (
    <Card
      id="limiter"
      title={t("リミッター")}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {v.safeParse(limiterSchema, settings).success ? (
        <>
          <TransferPlot
            threshold={settings.ceilingDb}
            ratio={Infinity}
            description={t("リミッターの設定上限による静特性。リリースの時間応答は含みません。")}
          />
          <Value>
            {t("上限")}
            {num(settings.ceilingDb)}
            {t("dBFS · リリース")}
            {num(settings.releaseMs)} ms
          </Value>
          <small>
            {t(
              "前段のサンプルピークを抑えます。後段の処理後のピークを保証するものではありません。",
            )}
          </small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function DereverbCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.dereverb);
  const setEnabled = useSettingsStore((state) => state.dereverb.setEnabled);
  return (
    <Card
      id="dereverb"
      title={t("反響低減")}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {v.safeParse(dereverbSchema, settings).success ? (
        <>
          <Plot
            description={t(
              "残響末尾を抑える強さの設定目安。直接音と反響の分離や実測抑制量を表すものではありません。",
            )}
            xLabel={t("抑制強度 0 → 100%（模式図）")}
            yLabel={t("設定量")}
          >
            <rect className={styles["filter-area"]} x="36" y="35" width="272" height="30" />
            <path className={styles["filter-curve"]} d={`M36 50H${36 + settings.amount * 272}`} />
            <circle
              className={styles["filter-point"]}
              cx={36 + settings.amount * 272}
              cy="50"
              r="4"
            />
          </Plot>
          <Value>
            {t("抑制強度")}
            {num(settings.amount * 100)}
            {t("% · 減衰時間")}
            {num(settings.decayMs)} ms
          </Value>
          <small>
            {t(
              "入力の包絡に基づき音の末尾を弱める簡易方式です。直接音や小声にも影響する場合があります。",
            )}
          </small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}

export function ReverbCard({ onSelect, locked }: CardProps) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.reverb);
  const setEnabled = useSettingsStore((state) => state.reverb.setEnabled);
  const valid = v.safeParse(reverbSchema, settings).success;
  const duration = settings.preDelayMs + settings.decayMs;
  const x = (time: number) => 36 + (time / duration) * 272;
  const points: [number, number][] = valid
    ? Array.from({ length: 81 }, (_, i) => [
        x(settings.preDelayMs + (settings.decayMs * i) / 80),
        86 - 64 * settings.mix * 10 ** ((-3 * i) / 80),
      ])
    : [];
  return (
    <Card
      id="reverb"
      title={t("リバーブ")}
      toggles={[{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]}
      onSelect={onSelect}
      locked={locked}
    >
      {valid ? (
        <>
          <Plot
            description={t(
              "プリディレイと残響減衰の設定イメージ。実測インパルス応答ではありません。",
            )}
            xLabel={t("0 → {0} ms（模式図）", { v0: num(duration) })}
            yLabel={t("残響の目安")}
          >
            <path className={styles["filter-reference"]} d={`M${x(settings.preDelayMs)} 22V86`} />
            <path className={styles["filter-curve"]} d={curvePath(points)} />
          </Plot>
          <Value>
            {t("ウェット")}
            {num(settings.mix * 100)}
            {t("% · プリディレイ")}
            {num(settings.preDelayMs)}
            {t("ms · 減衰")} {num(settings.decayMs)} ms
          </Value>
          <small>
            {t("図は設定から描いた減衰イメージです。停止後の残響は次の開始へ持ち越しません。")}
          </small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}
