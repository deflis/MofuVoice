import { useNoiseGateSettings } from "../settingsHooks";

export function NoiseGateSettings() {
  const { gate, gateThreshold, gateAttackMs, gateReleaseMs, update } = useNoiseGateSettings();
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
        <label>
          Threshold · dB / auto
          <input
            value={gateThreshold}
            onChange={(event) => update("gateThreshold", event.target.value)}
          />
        </label>
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
