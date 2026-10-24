import styles from "../settings.module.css";
import { NumberField } from "../NumberField";
import { DetailsPopup } from "../DetailsPopup";
import { useAutoLevelSettings } from "./hooks";
export function AutoLevelSettings({ part = "both" }: { part?: "both" | "auto" | "compressor" }) {
  const { level, levelDefaults, resetLevel, resetCompressor } = useAutoLevelSettings();
  return (
    <>
      {part !== "compressor" && (
        <section>
          <h2>後処理の音量自動調整</h2>
          <p>声の平均音量を目標に近づけます。</p>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={level.enabled}
              onChange={(event) => level.setEnabled(event.target.checked)}
            />
            有効
          </label>
          <fieldset className={styles.settings} disabled={!level.enabled} hidden={!level.enabled}>
            <DetailsPopup title="音量の自動調整" buttonLabel="詳細設定">
              <div className={styles["level-fields"]}>
                <NumberField
                  label="目標音量（dBFS・RMS）"
                  value={level.targetDb}
                  onChange={level.setTargetDb}
                  min={-36}
                  max={-6}
                  step={"any"}
                />
                <NumberField
                  label="最大増幅量（dB）"
                  value={level.maxGainDb}
                  onChange={level.setMaxGainDb}
                  min={0}
                  max={18}
                  step={"any"}
                />
                <NumberField
                  label="音声検出しきい値（dBFS）"
                  value={level.floorDb}
                  onChange={level.setFloorDb}
                  min={-90}
                  max={-30}
                  step={"any"}
                />
                <NumberField
                  label="音量調整の応答時間（ms）"
                  value={level.responseMs}
                  onChange={level.setResponseMs}
                  min={100}
                  max={10000}
                  step={"any"}
                />
              </div>
              <button type="button" disabled={!levelDefaults} onClick={resetLevel}>
                標準に戻す
              </button>
            </DetailsPopup>
          </fieldset>
        </section>
      )}
      {part !== "auto" && (
        <section>
          <h2>後処理コンプレッサー</h2>
          <p>大きな声を抑え、音量差を小さくします。</p>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={level.compressorEnabled}
              onChange={(event) => level.setCompressorEnabled(event.target.checked)}
            />
            有効
          </label>
          <fieldset
            className={styles.settings}
            disabled={!level.compressorEnabled}
            hidden={!level.compressorEnabled}
          >
            <DetailsPopup title="コンプレッサー" buttonLabel="詳細設定">
              <div className={styles["level-fields"]}>
                <NumberField
                  label="圧縮しきい値（dBFS）"
                  value={level.thresholdDb}
                  onChange={level.setThresholdDb}
                  min={-36}
                  max={0}
                  step={"any"}
                />
                <NumberField
                  label="圧縮比"
                  value={level.ratio}
                  onChange={level.setRatio}
                  min={1}
                  max={20}
                  step={"any"}
                />
                <NumberField
                  label="アタック（ms）"
                  value={level.attackMs}
                  onChange={level.setAttackMs}
                  min={0.1}
                  max={100}
                  step={"any"}
                />
                <NumberField
                  label="リリース（ms）"
                  value={level.releaseMs}
                  onChange={level.setReleaseMs}
                  min={10}
                  max={2000}
                  step={"any"}
                />
              </div>
              <button type="button" disabled={!levelDefaults} onClick={resetCompressor}>
                標準に戻す
              </button>
            </DetailsPopup>
          </fieldset>
        </section>
      )}
    </>
  );
}
