import { useNoiseGateSettings } from "../settingsHooks";

export function NoiseGateSettings() {
  const { gate, gateThresholdMode, gateThresholdDb, gateAttackMs, gateReleaseMs, update } =
    useNoiseGateSettings();
  const automatic = gateThresholdMode === "auto";
  return (
    <section>
      <div className="section-heading">
        <h2>ノイズゲート</h2>
        <label className="check">
          <input
            type="checkbox"
            checked={gate}
            onChange={(event) => update("gate", event.target.checked)}
          />
          有効
        </label>
      </div>
      <div className="gate-fields">
        <div>
          <label>
            境界の設定
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
            <small>背景ノイズに合わせて境界を自動調整します。</small>
          ) : (
            <label>
              境界 · dBFS
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
        <label>
          Attack · ms
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
          Release · ms
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
    </section>
  );
}
