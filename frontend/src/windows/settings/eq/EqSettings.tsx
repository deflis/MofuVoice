import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import eqStyles from "./EqSettings.module.css";
import type { EqKey } from "./slice";
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

export function EqSettings({
  filter = "eq",
  title = "変換後EQ",
}: {
  filter?: EqKey;
  title?: string;
}) {
  const { t } = useTranslation();
  const eq = useEqSettings(filter);
  const id = useId();
  const headroom = eq.bands.reduce((sum, band) => sum + Math.max(0, band.gainDb), 0);
  const validGain = eq.bands.every(
    (band) => Number.isFinite(band.gainDb) && Math.abs(band.gainDb) <= 12,
  );
  return (
    <section aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`}>{t(title)}</h2>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={eq.enabled}
          onChange={(event) => eq.setEnabled(event.target.checked)}
        />
        {t("有効")}
      </label>
      <fieldset className={eqStyles["eq-controls"]} disabled={!eq.enabled} hidden={!eq.enabled}>
        <>
          {eq.bands.map((band, index) => (
            <EqSimpleBand
              key={index}
              title={
                index < simpleBands.length
                  ? t(simpleBands[index].title)
                  : t("バンド{0}", { v0: index + 1 })
              }
              left={index < simpleBands.length ? t(simpleBands[index].left) : t("抑える")}
              right={index < simpleBands.length ? t(simpleBands[index].right) : t("強める")}
              value={band.gainDb}
              onChange={(value) => eq.setBandGainDb(index, value)}
            />
          ))}
          {!validGain && (
            <p className={styles.error}>
              {t("詳細設定でゲインを−12〜12 dBの数値に直すか、EQをリセットしてください。")}
            </p>
          )}
        </>

        <DetailsPopup
          title={t("音色の調整（EQ）")}
          buttonLabel={t("詳細設定")}
          onOpen={() => eq.setMode("pro")}
        >
          <>
            {eq.outputDb !== 0 && (
              <p>
                {t("クリップ防止の出力調整：")}
                {eq.outputDb}
                {t("dB（詳細設定で変更できます）")}
              </p>
            )}

            <p>
              {t(
                "ピーキングEQです。帯域幅（Q値）が小さいほど広い帯域、大きいほど狭い帯域に効きます。",
              )}
            </p>
            {eq.bands.map((band, index) => (
              <EqBandDetails
                key={index}
                title={t("バンド{0}・{1} Hz", { v0: index + 1, v1: band.frequency })}
                band={band}
                setFrequency={(value) => eq.setBandFrequency(index, value)}
                setGainDb={(value) => eq.setBandGainDb(index, value)}
                setQ={(value) => eq.setBandQ(index, value)}
              />
            ))}
            <label>
              {t("クリップ防止の出力調整（dB）")}
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
              ? t("ブーストに合わせて出力を{0} dB下げ、音量の余裕を確保します。", {
                  v0: headroom.toFixed(1),
                })
              : t("ブースト量の表示には有効なゲインが必要です。")}{" "}
            {t("小さく聞こえる場合は後段の音量の自動調整で調整してください。")}
          </p>
          <button type="button" onClick={eq.reset}>
            {t("EQの補正量を0に戻す")}
          </button>
          <p>
            {t(
              "全バンドの補正とクリップ防止の出力調整を0に戻します。周波数と帯域幅（Q値）はそのままです。",
            )}
          </p>
        </DetailsPopup>
      </fieldset>
    </section>
  );
}
