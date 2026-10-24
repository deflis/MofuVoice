import styles from "../../../styles.module.css";
import { NumberInput } from "../NumberInput";
import { EqSimpleBand, EqBandDetails } from "./BandSettings";
import { DetailsPopup } from "../DetailsPopup";
import { useId } from "react";
import { useEqSettings } from "./hooks";

const simpleBands = [
  { title: "低域の厚み", left: "すっきり", right: "ふっくら" },
  { title: "明瞭さ", left: "やわらかく", right: "くっきり" },
  { title: "明るさ", left: "落ち着いた", right: "明るい" },
] as const;

export function EqSettings() {
  const eq = useEqSettings();
  const id = useId();
  const headroom = eq.bands.reduce((sum, band) => sum + Math.max(0, band.gainDb), 0);
  const validGain = eq.bands.every(
    (band) => Number.isFinite(band.gainDb) && Math.abs(band.gainDb) <= 12,
  );
  return (
    <section aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`}>声の音色（EQ）</h2>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={eq.enabled}
          onChange={(event) => eq.setEnabled(event.target.checked)}
        />
        有効
      </label>
      <fieldset className={styles["eq-controls"]} disabled={!eq.enabled} hidden={!eq.enabled}>
        <>
          {eq.bands.map((band, index) => (
            <EqSimpleBand
              key={index}
              title={index < simpleBands.length ? simpleBands[index].title : `バンド${index + 1}`}
              left={index < simpleBands.length ? simpleBands[index].left : "抑える"}
              right={index < simpleBands.length ? simpleBands[index].right : "強める"}
              value={band.gainDb}
              onChange={(value) => eq.setBandGainDb(index, value)}
            />
          ))}
          {!validGain && (
            <p className={styles.error}>
              詳細設定でゲインを−12〜12 dBの数値に直すか、EQをリセットしてください。
            </p>
          )}
        </>

        <DetailsPopup
          title="音色の調整（EQ）"
          buttonLabel="詳細設定"
          onOpen={() => eq.setMode("pro")}
        >
          <>
            {eq.outputDb !== 0 && <p>出力の追加減衰：{eq.outputDb} dB。詳細設定で変更できます。</p>}

            <p>ピーキングEQです。Qが小さいほど広い帯域、大きいほど狭い帯域に効きます。</p>
            {eq.bands.map((band, index) => (
              <EqBandDetails
                key={index}
                title={`バンド${index + 1}・${band.frequency} Hz`}
                band={band}
                setFrequency={(value) => eq.setBandFrequency(index, value)}
                setGainDb={(value) => eq.setBandGainDb(index, value)}
                setQ={(value) => eq.setBandQ(index, value)}
              />
            ))}
            <label>
              出力の追加減衰（dB）
              <NumberInput
                required
                min="-18"
                max="0"
                step="0.1"
                value={eq.outputDb}
                onChange={(event) => eq.setOutputDb(event.currentTarget.valueAsNumber)}
              />
            </label>
          </>
          <p>
            {validGain
              ? `ブーストに合わせて出力を${headroom.toFixed(1)} dB下げ、音量の余裕を確保します。`
              : "ブースト量の表示には有効なゲインが必要です。"}{" "}
            小さく聞こえる場合は後段の音量の自動調整で調整してください。
          </p>
          <button type="button" onClick={eq.reset}>
            EQの補正量を0に戻す
          </button>
          <p>全バンドの補正と追加減衰を0に戻します。周波数・Qはそのままです。</p>
        </DetailsPopup>
      </fieldset>
    </section>
  );
}
