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
      <div className="section-heading">
        <h2>話していないときの雑音を減らす（ノイズゲート）</h2>
        <label className="check">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
          />
          有効
        </label>
      </div>
      <fieldset className="settings" disabled={!enabled} hidden={!enabled}>
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
                  <small>0に近づけるほど小さい音を強く抑えます。</small>
                </label>
              )}
            </div>
          </div>

          <p>アタックは音が通り始める速さ、リリースは話し終わったあとに音を閉じる速さです。</p>
          <div className="level-fields">
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
