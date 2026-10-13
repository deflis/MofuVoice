import { useId } from "react";
import { useEqSettings } from "../settingsHooks";
import type { EqBand } from "../types";

const bands = [
  {
    title: "低域の厚み",
    left: "すっきり",
    right: "ふっくら",
    help: "こもる声は左へ。声が薄いと感じたら右へ。",
  },
  {
    title: "明瞭さ",
    left: "やわらかく",
    right: "くっきり",
    help: "言葉がぼやけるときは右へ。硬く聞こえたら左へ。",
  },
  {
    title: "明るさ",
    left: "落ち着いた",
    right: "明るい",
    help: "明るくしたいときは右へ。サ行が刺さるときは左へ。",
  },
] as const;
const fields: { key: keyof EqBand; label: string; min: number; max: number; step: number }[] = [
  { key: "frequency", label: "周波数（Hz）", min: 40, max: 16000, step: 1 },
  { key: "gainDb", label: "ゲイン（dB）", min: -12, max: 12, step: 0.1 },
  { key: "q", label: "Q", min: 0.2, max: 8, step: 0.1 },
];

export function EqSettings() {
  const { eq, update, updateBand, reset } = useEqSettings();
  const id = useId();
  const headroom = eq.bands.reduce((sum, band) => sum + Math.max(0, Number(band.gainDb)), 0);
  const validGain = eq.bands.every(
    (band) =>
      band.gainDb.trim() !== "" &&
      Number.isFinite(Number(band.gainDb)) &&
      Math.abs(Number(band.gainDb)) <= 12,
  );
  const custom = eq.bands.some(
    (band, index) =>
      Number(band.frequency) !== [250, 3000, 8000][index] ||
      Number(band.q) !== [0.7, 0.8, 0.7][index],
  );
  return (
    <section aria-labelledby={`${id}-title`} className="eq-section">
      <h2 id={`${id}-title`}>声の音色（EQ）</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={eq.enabled}
          onChange={(event) => update("eq", { ...eq, enabled: event.target.checked })}
        />
        EQを使う
      </label>
      <fieldset className="eq-modes" aria-label="EQの操作モード">
        <button
          type="button"
          aria-pressed={eq.mode === "simple"}
          onClick={() => update("eq", { ...eq, mode: "simple" })}
        >
          簡単モード
        </button>
        <button
          type="button"
          aria-pressed={eq.mode === "pro"}
          onClick={() => update("eq", { ...eq, mode: "pro" })}
        >
          プロ向けモード
        </button>
      </fieldset>
      <p>切り替えても音色と設定はそのままです。変更は停止してから行い、Startで適用します。</p>
      <fieldset className="eq-controls" disabled={!eq.enabled}>
        {eq.mode === "simple" ? (
          <>
            <p>
              まずは中央の0から、1〜3 dBずつ調整してください。EQのオン・オフで聞き比べられます。
            </p>
            {custom && (
              <p className="notice">
                プロ向けモードで設定した周波数・Qを使っています。ここでは各バンドの強さだけを調整します。
              </p>
            )}
            {([0, 1, 2] as const).map((index) => {
              const i = index;
              const band = bands[index];
              const value = eq.bands[index].gainDb;
              const valid =
                value.trim() !== "" &&
                Number.isFinite(Number(value)) &&
                Math.abs(Number(value)) <= 12;
              return (
                <div className="eq-simple-band" key={band.title}>
                  <label htmlFor={`${id}-${i}`}>
                    {band.title}{" "}
                    <output>
                      {valid
                        ? `${Number(value) > 0 ? "+" : ""}${value} dB`
                        : "値を確認してください"}
                    </output>
                  </label>
                  <input
                    id={`${id}-${i}`}
                    type="range"
                    min="-12"
                    max="12"
                    step="0.5"
                    value={valid ? Number(value) : 0}
                    disabled={!valid}
                    onChange={(event) => updateBand(index, "gainDb", event.target.value)}
                  />
                  <div className="eq-scale">
                    <span>{band.left}</span>
                    <span>0：補正なし</span>
                    <span>{band.right}</span>
                  </div>
                  <p>{band.help}</p>
                </div>
              );
            })}
            {!validGain && (
              <p className="error">
                プロ向けモードでゲインを−12〜12 dBの数値に直すか、EQをリセットしてください。
              </p>
            )}
            {Number(eq.outputDb) !== 0 && (
              <p>出力の追加減衰：{eq.outputDb} dB。プロ向けモードで変更できます。</p>
            )}
          </>
        ) : (
          <>
            <p>3バンドのピーキングEQです。Qが小さいほど広い帯域、大きいほど狭い帯域に効きます。</p>
            {([0, 1, 2] as const).map((i) => (
              <fieldset className="eq-pro-band" key={i}>
                <legend>
                  バンド{i + 1}・{bands[i].title}
                </legend>
                <div className="eq-fields">
                  {fields.map((field) => (
                    <label key={field.key}>
                      {field.label}
                      <input
                        type="number"
                        required
                        min={field.min}
                        max={field.max}
                        step={field.step}
                        value={eq.bands[i][field.key]}
                        onChange={(event) => updateBand(i, field.key, event.target.value)}
                      />
                    </label>
                  ))}
                </div>
              </fieldset>
            ))}
            <label>
              出力の追加減衰（dB）
              <input
                type="number"
                required
                min="-18"
                max="0"
                step="0.1"
                value={eq.outputDb}
                onChange={(event) => update("eq", { ...eq, outputDb: event.target.value })}
              />
            </label>
          </>
        )}
      </fieldset>
      <p>
        {validGain
          ? `ブーストに合わせて出力を${headroom.toFixed(1)} dB下げ、音量の余裕を確保します。`
          : "ブースト量の表示には有効なゲインが必要です。"}{" "}
        小さく聞こえる場合は後段のAuto levelで調整してください。
      </p>
      <button type="button" onClick={reset}>
        EQだけリセット
      </button>
      <p>リセットすると全バンドの補正と追加減衰が0になり、周波数・Qも既定値に戻ります。</p>
    </section>
  );
}
