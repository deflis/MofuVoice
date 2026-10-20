import { useNoiseGateSettings } from "./hooks";

export function RumbleSettings() {
  const { rumbleFilter, setRumbleFilter } = useNoiseGateSettings();
  return (
    <section>
      <h2>マイクの吹かれ・振動ノイズを減らす（ランブルフィルター）</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={rumbleFilter}
          onChange={(event) => setRumbleFilter(event.target.checked)}
        />
        有効
      </label>
    </section>
  );
}
