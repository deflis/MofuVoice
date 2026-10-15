import { useDeviceSelection } from "./hooks";

export function DeviceSelection() {
  const { device, outputDevice, resolvedBackend, devices, outputDevices, update } =
    useDeviceSelection();
  return (
    <>
      <label>
        {resolvedBackend === "asio" ? "入出力デバイス" : "入力デバイス"}
        <select value={device} onChange={(event) => update("device", event.target.value)}>
          <option value="">
            {resolvedBackend === "asio" ? "デバイスを選択" : "システムの既定"}
          </option>
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
            <option value="">システムの既定</option>
            {outputDevice && !outputDevices.includes(outputDevice) && (
              <option value={outputDevice}>{outputDevice}（未検出）</option>
            )}
            {outputDevices.map((available) => (
              <option key={available}>{available}</option>
            ))}
          </select>
        </label>
      )}
    </>
  );
}
