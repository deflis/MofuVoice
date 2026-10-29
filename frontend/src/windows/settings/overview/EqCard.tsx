import { useTranslation } from "react-i18next";
import styles from "./overview.module.css";
import * as v from "valibot";
import { useSettingsStore } from "../store";
import { eqSchema } from "../eq/schema";
import { Card, InvalidGraph, Value, num, type CardProps } from "./Card";
import { Plot, curvePath } from "./Plot";
import { eqResponseDb, rumbleResponseDb } from "./eqResponse";

const x = (frequency: number) => 36 + (Math.log10(frequency / 20) / 3) * 272;

export function EqCard({
  slot,
  id,
  title,
  onSelect,
  locked,
}: CardProps & {
  slot: "eq" | "inputEq" | "preConversionEq";
  id: string;
  title: string;
}) {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state[slot]);
  const rate = useSettingsStore((state) => state.devices.sampleRate);
  const rumble = useSettingsStore((state) => slot === "inputEq" && state.noise.rumbleFilter);
  const setEnabled = useSettingsStore((state) => state[slot].setEnabled);
  const setRumbleFilter = useSettingsStore((state) => state.noise.setRumbleFilter);
  const isInput = slot === "inputEq";
  const rateSupported = [44100, 48000].includes(rate);
  const validEq = v.safeParse(eqSchema, settings).success && rateSupported;
  const valid = (settings.enabled ? validEq : true) && (!rumble || rateSupported);
  const frequencies = Array.from({ length: 121 }, (_, i) => 20 * 1000 ** (i / 120));
  const response = (frequency: number) =>
    isInput
      ? (settings.enabled ? eqResponseDb(settings, rate, frequency) : 0) +
        (rumble ? rumbleResponseDb(rate, frequency) : 0)
      : eqResponseDb(settings, rate, frequency);
  const values = valid ? frequencies.map(response) : [];
  const minimum = Math.floor(Math.min(-12, ...values) / 6) * 6;
  const maximum = Math.ceil(Math.max(6, ...values) / 6) * 6;
  const y = (db: number) => 86 - ((db - minimum) / (maximum - minimum)) * 64;
  return (
    <Card
      id={id}
      title={t(title)}
      onSelect={onSelect}
      locked={locked}
      toggles={
        isInput
          ? [
              { label: "EQ", enabled: settings.enabled, onChange: setEnabled },
              { label: "80 Hz HPF", enabled: rumble, onChange: setRumbleFilter },
            ]
          : [{ label: t("有効"), enabled: settings.enabled, onChange: setEnabled }]
      }
    >
      {(settings.enabled || (isInput && rumble)) &&
        (valid ? (
          <>
            <Plot
              description={t("{0}の有効な処理の周波数応答。{1} Hz。{2}", {
                v0: t(title),
                v1: rate,
                v2: settings.enabled
                  ? t("EQの出力調整と正のゲインに対するヘッドルームを含みます。")
                  : t("80 Hz HPFの設定応答です。"),
              })}
              xLabel={t("20 Hz → 20 kHz（対数）")}
              yLabel={`${minimum}〜${maximum} dB`}
            >
              <path className={styles["filter-reference"]} d={`M36 ${y(0)}H308`} />
              <path
                className={styles["filter-curve"]}
                d={curvePath(frequencies.map((frequency, i) => [x(frequency), y(values[i])]))}
              />
              {settings.enabled &&
                settings.bands.map((band, i) => (
                  <circle
                    key={i}
                    className={styles["filter-point"]}
                    cx={x(band.frequency)}
                    cy={y(response(band.frequency))}
                    r="3"
                  />
                ))}
            </Plot>
            {isInput && rumble && <Value>{t("低域ノイズ対策: 80 Hz HPF")}</Value>}
            {settings.enabled && (
              <>
                <Value>
                  {settings.bands
                    .map(
                      (band) =>
                        `${num(band.frequency)} Hz: ${num(band.gainDb)} dB / Q ${num(band.q)}`,
                    )
                    .join(" · ")}
                </Value>
                <Value>
                  {t("出力調整")}
                  {num(settings.outputDb)}
                  {t("dB · ヘッドルーム")}{" "}
                  {num(settings.bands.reduce((sum, band) => sum + Math.max(0, band.gainDb), 0))} dB
                  · {rate.toLocaleString()} Hz
                </Value>
              </>
            )}
            <small>
              {isInput ? t("図は有効なHPFとEQの合成特性です。") : t("図はEQの設定周波数応答です。")}
              {settings.enabled && t("出力調整と内部ヘッドルームを含みます。")}
            </small>
          </>
        ) : (
          <InvalidGraph />
        ))}
    </Card>
  );
}
