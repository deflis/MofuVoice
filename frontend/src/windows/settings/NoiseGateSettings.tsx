import { DetailsPopup } from "./DetailsPopup";
import { useNoiseGateSettings } from "./hooks";

export function NoiseGateSettings() {
  const { gate, gateThresholdMode, gateThresholdDb, gateAttackMs, gateReleaseMs, update } =
    useNoiseGateSettings();
  const automatic = gateThresholdMode === "auto";
  return (
    <section>
      <div className="section-heading">
        <h2>話していないときの雑音を減らす（ノイズゲート）</h2>
        <label className="check">
          <input
            type="checkbox"
            checked={gate}
            onChange={(event) => update("gate", event.target.checked)}
          />
          有効
        </label>
      </div>
      <fieldset className="settings" disabled={!gate} hidden={!gate}>
        <DetailsPopup title="ノイズゲート" buttonLabel="詳細設定">
          <div>
            <div>
              <label>
                しきい値の設定方法
                <select
                  value={gateThresholdMode}
                  onChange={(event) => {
                    update("gateThresholdMode", event.target.value === "auto" ? "auto" : "manual");
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
                  <input
                    type="number"
                    min="-90"
                    max="0"
                    step="any"
                    required
                    value={gateThresholdDb}
                    onChange={(event) => update("gateThresholdDb", event.target.value)}
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
              <input
                type="number"
                min="0"
                step="any"
                required
                value={gateAttackMs}
                onChange={(event) => update("gateAttackMs", event.target.value)}
              />
            </label>
            <label>
              リリース（ms）
              <input
                type="number"
                min="0"
                step="any"
                required
                value={gateReleaseMs}
                onChange={(event) => update("gateReleaseMs", event.target.value)}
              />
            </label>
          </div>
        </DetailsPopup>
      </fieldset>
    </section>
  );
}
