import { useId } from "react";
import { useVoiceSettings } from "../settingsHooks";
import type { DeEsserParameters } from "../types";

const defaults: DeEsserParameters = {
  frequencyHz: "5000",
  thresholdDb: "-24",
  rangeDb: "6",
  attackMs: "0.5",
  releaseMs: "60",
};
const fields: {
  key: keyof DeEsserParameters;
  label: string;
  min: number;
  max: number;
  step: number;
  help: string;
}[] = [
  {
    key: "frequencyHz",
    label: "分割周波数（Hz）",
    min: 2000,
    max: 12000,
    step: 1,
    help: "この周波数より高い音を中心に抑えます。",
  },
  {
    key: "thresholdDb",
    label: "しきい値（dBFS）",
    min: -60,
    max: 0,
    step: 0.1,
    help: "低くするほど、小さいサ行にも反応します。",
  },
  {
    key: "rangeDb",
    label: "最大減衰量（dB）",
    min: 0,
    max: 12,
    step: 0.5,
    help: "大きくするほど強く抑えます。0で補正なし。",
  },
  {
    key: "attackMs",
    label: "アタック（ms）",
    min: 0.1,
    max: 20,
    step: 0.1,
    help: "小さくするほど素早く抑え始めます。",
  },
  {
    key: "releaseMs",
    label: "リリース（ms）",
    min: 10,
    max: 500,
    step: 1,
    help: "大きくするほど、ゆっくり元の音に戻ります。",
  },
];

export function DeEsserSettings() {
  const { deesser, deesserMode, deesserParameters: parameters, update } = useVoiceSettings();
  const id = useId();
  const value = Number(parameters.rangeDb);
  const valid =
    parameters.rangeDb.trim() !== "" && Number.isFinite(value) && value >= 0 && value <= 12;
  const custom = fields.some(
    (field) =>
      field.key !== "rangeDb" && Number(parameters[field.key]) !== Number(defaults[field.key]),
  );
  const change = (key: keyof DeEsserParameters, value: string) =>
    update("deesserParameters", { ...parameters, [key]: value });
  return (
    <section aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`}>サ行の刺さり（ディエッサー）</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={deesser}
          onChange={(event) => update("deesser", event.target.checked)}
        />
        サ行をやわらかくする
      </label>
      <fieldset className="deesser-modes" aria-label="ディエッサーの操作モード">
        <button
          type="button"
          aria-pressed={deesserMode === "simple"}
          onClick={() => update("deesserMode", "simple")}
        >
          簡単モード
        </button>
        <button
          type="button"
          aria-pressed={deesserMode === "pro"}
          onClick={() => update("deesserMode", "pro")}
        >
          プロ向けモード
        </button>
      </fieldset>
      <p>切り替えても音と設定はそのままです。変更は停止してから行い、Startで適用します。</p>
      <fieldset className="deesser-controls" disabled={!deesser}>
        {deesserMode === "simple" ? (
          <>
            <p>「さ・し・す」が耳に刺さるときに使います。まずは標準の位置で試してください。</p>
            {custom && (
              <p className="notice">
                プロ向けモードで調整した設定を使っています。ここでは抑える強さだけを変えます。
              </p>
            )}
            <div className="deesser-simple">
              <label htmlFor={`${id}-amount`}>
                抑える強さ{" "}
                <output>{valid ? `${parameters.rangeDb} dB` : "値を確認してください"}</output>
              </label>
              <input
                id={`${id}-amount`}
                type="range"
                min="0"
                max="12"
                step="0.5"
                value={valid ? value : 6}
                disabled={!valid}
                onChange={(event) => change("rangeDb", event.target.value)}
              />
              <div className="deesser-scale">
                <span>0：補正なし</span>
                <span>6：標準</span>
                <span>強く抑える</span>
              </div>
              <p>
                まだ刺さるときは右へ。声がこもるときは左へ。「さしすせそ」と話し、オン・オフで聞き比べてください。
              </p>
            </div>
            {!valid && (
              <p className="error">
                プロ向けモードで最大減衰量を0〜12 dBに直すか、ディエッサーをリセットしてください。
              </p>
            )}
          </>
        ) : (
          <>
            <p>
              高域の音量と、音全体に対する高域の割合を検出し、高域だけを動的に減衰します。圧縮比は4:1、先読みはありません。
            </p>
            <div className="deesser-fields">
              {fields.map((field) => (
                <label key={field.key}>
                  {field.label}
                  <input
                    type="number"
                    required
                    min={field.min}
                    max={field.max}
                    step={field.step}
                    value={parameters[field.key]}
                    onChange={(event) => change(field.key, event.target.value)}
                  />
                  <small>{field.help}</small>
                </label>
              ))}
            </div>
          </>
        )}
      </fieldset>
      <button type="button" onClick={() => update("deesserParameters", { ...defaults })}>
        ディエッサーだけリセット
      </button>
      <p>強さと詳細設定を標準に戻します。オン・オフと操作モードはそのままです。</p>
    </section>
  );
}
