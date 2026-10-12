import { useDeviceSettings } from "../settingsHooks";

export function DeviceSettings() {
  const { device, frames, devices, update } = useDeviceSettings();
  return (
    <section>
      <h2>音声デバイス</h2>
      <label>
        ASIO device
        <select value={device} onChange={(event) => update("device", event.target.value)}>
          <option value="">デバイスを選択</option>
          {device && !devices.includes(device) && (
            <option value={device}>{device}（未検出）</option>
          )}
          {devices.map((availableDevice) => (
            <option key={availableDevice}>{availableDevice}</option>
          ))}
        </select>
      </label>
      <label>
        Frames
        <input
          type="number"
          min="1"
          max="8192"
          step="1"
          required
          value={frames}
          onChange={(event) => update("frames", event.target.value)}
        />
      </label>
    </section>
  );
}
