import styles from "../settings.module.css";
import deesserStyles from "./DeEsserSettings.module.css";
import { NumberField } from "../NumberField";
import { DetailsPopup } from "../DetailsPopup";
import { useId } from "react";
import { useDeEsserSettings } from "./hooks";
import type { DeEsserParameters } from "../../../types";

const defaults: DeEsserParameters = {
  frequencyHz: 5000,
  thresholdDb: -24,
  rangeDb: 6,
  attackMs: 0.5,
  releaseMs: 60,
};
export function DeEsserSettings() {
  const parameters = useDeEsserSettings();
  const id = useId();
  const value = parameters.rangeDb;
  const valid = Number.isFinite(value) && value >= 0 && value <= 12;
  const custom =
    parameters.frequencyHz !== defaults.frequencyHz ||
    parameters.thresholdDb !== defaults.thresholdDb ||
    parameters.attackMs !== defaults.attackMs ||
    parameters.releaseMs !== defaults.releaseMs;
  return (
    <section aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`}>サ行をやわらかくする（ディエッサー）</h2>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={parameters.enabled}
          onChange={(event) => parameters.setEnabled(event.target.checked)}
        />
        有効
      </label>
      <fieldset
        className={deesserStyles["deesser-controls"]}
        disabled={!parameters.enabled}
        hidden={!parameters.enabled}
      >
        <>
          <div className={deesserStyles["deesser-simple"]}>
            <label htmlFor={`${id}-amount`}>抑える強さ </label>
            <input
              id={`${id}-amount`}
              type="range"
              min="0"
              max="12"
              step="0.5"
              value={valid ? value : 6}
              disabled={!valid}
              onChange={(event) => parameters.setRangeDb(event.currentTarget.valueAsNumber)}
            />
            <div className={deesserStyles["deesser-scale"]}>
              <span>弱く</span>
              <span>標準</span>
              <span>強く</span>
            </div>
          </div>
          {!valid && (
            <p className={styles.error}>
              詳細設定で最大減衰量を0〜12 dBに直すか、ディエッサーをリセットしてください。
            </p>
          )}
        </>

        <DetailsPopup
          title="ディエッサー"
          buttonLabel="詳細設定"
          onOpen={() => parameters.setMode("pro")}
        >
          <>
            {custom && (
              <p className={styles.notice}>
                詳細設定で調整した設定を使っています。ここでは抑える強さだけを変えます。
              </p>
            )}
            <p>
              まだ刺さるときは右へ。声がこもるときは左へ。「さしすせそ」と話し、オン・オフで聞き比べてください。
            </p>
            <p>
              高域の音量と、音全体に対する高域の割合を検出し、高域だけを動的に減衰します。圧縮比は4:1、先読みはありません。
            </p>
            <div className={deesserStyles["deesser-fields"]}>
              <NumberField
                label="分割周波数（Hz）"
                value={parameters.frequencyHz}
                onChange={parameters.setFrequencyHz}
                min={2000}
                max={12000}
                step={1}
                help="この周波数より高い音を中心に抑えます。"
              />
              <NumberField
                label="しきい値（dBFS）"
                value={parameters.thresholdDb}
                onChange={parameters.setThresholdDb}
                min={-60}
                max={0}
                step={0.1}
                help="低くするほど、小さいサ行にも反応します。"
              />
              <NumberField
                label="最大減衰量（dB）"
                value={parameters.rangeDb}
                onChange={parameters.setRangeDb}
                min={0}
                max={12}
                step={0.5}
                help="大きくするほど強く抑えます。0で補正なし。"
              />
              <NumberField
                label="アタック（ms）"
                value={parameters.attackMs}
                onChange={parameters.setAttackMs}
                min={0.1}
                max={20}
                step={0.1}
                help="小さくするほど素早く抑え始めます。"
              />
              <NumberField
                label="リリース（ms）"
                value={parameters.releaseMs}
                onChange={parameters.setReleaseMs}
                min={10}
                max={500}
                step={1}
                help="大きくするほど、ゆっくり元の音に戻ります。"
              />
            </div>
          </>
          <button
            type="button"
            onClick={() => {
              parameters.setFrequencyHz(defaults.frequencyHz);
              parameters.setThresholdDb(defaults.thresholdDb);
              parameters.setRangeDb(defaults.rangeDb);
              parameters.setAttackMs(defaults.attackMs);
              parameters.setReleaseMs(defaults.releaseMs);
            }}
          >
            ディエッサーを標準に戻す
          </button>
          <p>強さと詳細設定を標準に戻します。オン・オフと操作モードはそのままです。</p>
        </DetailsPopup>
      </fieldset>
    </section>
  );
}
