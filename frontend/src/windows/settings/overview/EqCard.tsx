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
}: CardProps & {
  slot: "eq" | "inputEq" | "preConversionEq";
  id: string;
  title: string;
}) {
  const settings = useSettingsStore((state) => state[slot]);
  const rate = useSettingsStore((state) => state.devices.sampleRate);
  const rumble = useSettingsStore((state) => slot === "inputEq" && state.noise.rumbleFilter);
  const isInput = slot === "inputEq";
  const valid = v.safeParse(eqSchema, settings).success && [44100, 48000].includes(rate);
  const frequencies = Array.from({ length: 121 }, (_, i) => 20 * 1000 ** (i / 120));
  const response = (frequency: number) =>
    isInput
      ? (settings.enabled ? eqResponseDb(settings, rate, frequency) : 0) +
        (rumble ? rumbleResponseDb(rate, frequency) : 0)
      : eqResponseDb(settings, rate, frequency);
  const retained = valid
    ? frequencies.map((frequency) => eqResponseDb(settings, rate, frequency))
    : [];
  const values = valid ? frequencies.map(response) : [];
  const minimum = Math.floor(Math.min(-12, ...values, ...retained) / 6) * 6;
  const maximum = Math.ceil(Math.max(6, ...values, ...retained) / 6) * 6;
  const y = (db: number) => 86 - ((db - minimum) / (maximum - minimum)) * 64;
  return (
    <Card
      id={id}
      title={title}
      enabled={settings.enabled || (isInput && rumble)}
      onSelect={onSelect}
    >
      {isInput && (
        <Value>
          低域ノイズ対策（80 Hz HPF）: {rumble ? "有効" : "無効"} · EQ:{" "}
          {settings.enabled ? "有効" : "無効・設定保持"}
        </Value>
      )}
      {valid ? (
        <>
          <Plot
            description={`${title}の設定周波数応答。${rate} Hz。出力調整と正のゲインに対するヘッドルームを含む。${isInput ? "実線は有効なHPFとEQの合成特性。破線は保持中のEQ単体特性。" : "無効時も設定特性を表示。"}`}
            xLabel="20 Hz → 20 kHz（対数）"
            yLabel={`${minimum}〜${maximum} dB`}
          >
            <path className={styles["filter-reference"]} d={`M36 ${y(0)}H308`} />
            {isInput && (
              <path
                className={styles["filter-retained"]}
                d={curvePath(frequencies.map((frequency, i) => [x(frequency), y(retained[i])]))}
              />
            )}
            <path
              className={styles["filter-curve"]}
              d={curvePath(frequencies.map((frequency, i) => [x(frequency), y(values[i])]))}
            />
            {settings.bands.map((band, i) => (
              <circle
                key={i}
                className={styles["filter-point"]}
                cx={x(band.frequency)}
                cy={y(response(band.frequency))}
                r="3"
              />
            ))}
          </Plot>
          <Value>
            {settings.bands
              .map((band) => `${num(band.frequency)} Hz: ${num(band.gainDb)} dB / Q ${num(band.q)}`)
              .join(" · ")}
          </Value>
          <Value>
            出力調整 {num(settings.outputDb)} dB · ヘッドルーム{" "}
            {num(settings.bands.reduce((sum, band) => sum + Math.max(0, band.gainDb), 0))} dB ·{" "}
            {rate.toLocaleString()} Hz
          </Value>
          <small>
            {isInput
              ? "実線: 有効なHPFとEQの合成。破線: 保持中のEQ単体設定。"
              : "設定特性（無効時も保持）。"}
            出力調整と内部ヘッドルームを含みます。
          </small>
        </>
      ) : (
        <InvalidGraph />
      )}
    </Card>
  );
}
