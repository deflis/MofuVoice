import styles from "../settings.module.css";
import { NumberInput } from "../NumberInput";
import { DetailsPopup } from "../DetailsPopup";
import { useNoiseGateSettings } from "./hooks";

export function NoiseGateSettings() {
  const {
    enabled,
    thresholdMode,
    thresholdDb,
    attackMs,
    releaseMs,
    setEnabled,
    setThresholdMode,
    setThresholdDb,
    setAttackMs,
    setReleaseMs,
  } = useNoiseGateSettings();
  const automatic = thresholdMode === "auto";
  return (
    <section>
      <div>
        <h2>話していないときの雑音を減らす（ノイズゲート）</h2>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
          />
          有効
        </label>
      </div>
      <fieldset className={styles.settings} disabled={!enabled} hidden={!enabled}>
        <DetailsPopup title="ノイズゲート" buttonLabel="詳細設定">
          <div>
            <div>
              <label>
                しきい値の設定方法
                <select
                  value={thresholdMode}
                  onChange={(event) => {
                    setThresholdMode(event.target.value === "auto" ? "auto" : "manual");
                  }}
                >
                  <option value="auto">自動</option>
                  <option value="manual">手動</option>
                </select>
              </label>
              {automatic ? (
                <small>背景ノイズに合わせてしきい値を自動調整します。</small>
              ) : (
                <label>
                  しきい値（dBFS）
                  <NumberInput
                    min="-90"
                    max="0"
                    step="any"
                    required
                    value={thresholdDb}
                    onChange={(event) => setThresholdDb(event.currentTarget.valueAsNumber)}
                  />
                  <small>
                    0
                    dBFSに近づけるほど、より大きな声を出さないと音が出なくなります（雑音を強力に遮断します）。
                  </small>
                </label>
              )}
            </div>
          </div>

          <p>
            アタックは声が出たときにゲートが開く速さ、リリースは話し終わったあとにゲートが閉じる速さです。
          </p>
          <div className={styles["level-fields"]}>
            <label>
              アタック（ms）
              <NumberInput
                min="0.1"
                step="any"
                required
                value={attackMs}
                onChange={(event) => setAttackMs(event.currentTarget.valueAsNumber)}
              />
            </label>
            <label>
              リリース（ms）
              <NumberInput
                min="0.1"
                step="any"
                required
                value={releaseMs}
                onChange={(event) => setReleaseMs(event.currentTarget.valueAsNumber)}
              />
            </label>
          </div>
        </DetailsPopup>
      </fieldset>
    </section>
  );
}
