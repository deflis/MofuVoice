import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import { NumberField } from "../NumberField";
import { DetailsPopup } from "../DetailsPopup";
import { useAutoLevelSettings } from "./hooks";
export function AutoLevelSettings({ part = "both" }: { part?: "both" | "auto" | "compressor" }) {
  const { t } = useTranslation();
  const { level, levelDefaults, resetLevel, resetCompressor } = useAutoLevelSettings();
  return (
    <>
      {part !== "compressor" && (
        <section>
          <h2>{t("後処理の音量自動調整")}</h2>
          <p>{t("声の平均音量を目標に近づけます。")}</p>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={level.enabled}
              onChange={(event) => level.setEnabled(event.target.checked)}
            />
            {t("有効")}
          </label>
          <fieldset className={styles.settings} disabled={!level.enabled} hidden={!level.enabled}>
            <DetailsPopup title={t("音量の自動調整")} buttonLabel={t("詳細設定")}>
              <div className={styles["level-fields"]}>
                <NumberField
                  label={t("目標音量（dBFS・RMS）")}
                  value={level.targetDb}
                  onChange={level.setTargetDb}
                  min={-36}
                  max={-6}
                  step={"any"}
                />
                <NumberField
                  label={t("最大増幅量（dB）")}
                  value={level.maxGainDb}
                  onChange={level.setMaxGainDb}
                  min={0}
                  max={18}
                  step={"any"}
                />
                <NumberField
                  label={t("音声検出しきい値（dBFS）")}
                  value={level.floorDb}
                  onChange={level.setFloorDb}
                  min={-90}
                  max={-30}
                  step={"any"}
                />
                <NumberField
                  label={t("音量調整の応答時間（ms）")}
                  value={level.responseMs}
                  onChange={level.setResponseMs}
                  min={100}
                  max={10000}
                  step={"any"}
                />
              </div>
              <button type="button" disabled={!levelDefaults} onClick={resetLevel}>
                {t("標準に戻す")}
              </button>
            </DetailsPopup>
          </fieldset>
        </section>
      )}
      {part !== "auto" && (
        <section>
          <h2>{t("後処理コンプレッサー")}</h2>
          <p>{t("大きな声を抑え、音量差を小さくします。")}</p>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={level.compressorEnabled}
              onChange={(event) => level.setCompressorEnabled(event.target.checked)}
            />
            {t("有効")}
          </label>
          <fieldset
            className={styles.settings}
            disabled={!level.compressorEnabled}
            hidden={!level.compressorEnabled}
          >
            <DetailsPopup title={t("コンプレッサー")} buttonLabel={t("詳細設定")}>
              <div className={styles["level-fields"]}>
                <NumberField
                  label={t("圧縮しきい値（dBFS）")}
                  value={level.thresholdDb}
                  onChange={level.setThresholdDb}
                  min={-36}
                  max={0}
                  step={"any"}
                />
                <NumberField
                  label={t("圧縮比")}
                  value={level.ratio}
                  onChange={level.setRatio}
                  min={1}
                  max={20}
                  step={"any"}
                />
                <NumberField
                  label={t("アタック（ms）")}
                  value={level.attackMs}
                  onChange={level.setAttackMs}
                  min={0.1}
                  max={100}
                  step={"any"}
                />
                <NumberField
                  label={t("リリース（ms）")}
                  value={level.releaseMs}
                  onChange={level.setReleaseMs}
                  min={10}
                  max={2000}
                  step={"any"}
                />
              </div>
              <button type="button" disabled={!levelDefaults} onClick={resetCompressor}>
                {t("標準に戻す")}
              </button>
            </DetailsPopup>
          </fieldset>
        </section>
      )}
    </>
  );
}
