import { useDeviceSettings } from "../settingsHooks";

export function DeviceSettings() {
  const {
    device,
    outputDevice,
    backend,
    resolvedBackend,
    frames,
    devices,
    outputDevices,
    backends,
    update,
    updateBackend,
  } = useDeviceSettings();
  return (
    <section>
      <h2>音声デバイス</h2>
      <label>
        音声API
        <select value={backend} onChange={(event) => void updateBackend(event.target.value)}>
          {!backends.includes(backend) && <option value={backend}>{backend}（利用不可）</option>}
          {backends.map((available) => (
            <option key={available} value={available}>
              {{ auto: "自動（ASIO優先）", cpal: "標準（CPAL）", asio: "ASIO" }[available]}
            </option>
          ))}
        </select>
      </label>
      <label>
        {resolvedBackend === "asio" ? "ASIO device" : "入力デバイス"}
        <select value={device} onChange={(event) => update("device", event.target.value)}>
          <option value="">{resolvedBackend === "asio" ? "デバイスを選択" : "OSの既定入力"}</option>
          {device && !devices.includes(device) && (
            <option value={device}>{device}（未検出）</option>
          )}
          {devices.map((availableDevice) => (
            <option key={availableDevice}>{availableDevice}</option>
          ))}
        </select>
      </label>
      {resolvedBackend === "cpal" && (
        <label>
          出力デバイス
          <select
            value={outputDevice}
            onChange={(event) => update("outputDevice", event.target.value)}
          >
            <option value="">OSの既定出力</option>
            {outputDevice && !outputDevices.includes(outputDevice) && (
              <option value={outputDevice}>{outputDevice}（未検出）</option>
            )}
            {outputDevices.map((available) => (
              <option key={available}>{available}</option>
            ))}
          </select>
        </label>
      )}
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
