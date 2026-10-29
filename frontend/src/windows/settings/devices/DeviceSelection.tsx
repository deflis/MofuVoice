import { useTranslation } from "react-i18next";
import { useDeviceSelection } from "./hooks";

export function DeviceSelection() {
  const { t } = useTranslation();
  const {
    device,
    outputDevice,
    previewDevice,
    setPreviewDevice,
    resolvedBackend,
    devices,
    outputDevices,
    setDevice,
    setOutputDevice,
  } = useDeviceSelection();
  return (
    <>
      <label>
        {resolvedBackend === "asio" ? t("入出力デバイス") : t("入力デバイス")}
        <select value={device} onChange={(event) => setDevice(event.target.value)}>
          <option value="">
            {resolvedBackend === "asio" ? t("デバイスを選択") : t("システムの既定")}
          </option>
          {device && !devices.includes(device) && (
            <option value={device}>
              {device}
              {t("（未検出）")}
            </option>
          )}
          {devices.map((availableDevice) => (
            <option key={availableDevice}>{availableDevice}</option>
          ))}
        </select>
      </label>
      {resolvedBackend === "cpal" && (
        <>
          <label>
            {t("出力デバイス")}
            <select value={outputDevice} onChange={(event) => setOutputDevice(event.target.value)}>
              <option value="">{t("システムの既定")}</option>
              {outputDevice && !outputDevices.includes(outputDevice) && (
                <option value={outputDevice}>
                  {outputDevice}
                  {t("（未検出）")}
                </option>
              )}
              {outputDevices.map((available) => (
                <option key={available}>{available}</option>
              ))}
            </select>
          </label>
          <label>
            {t("プレビュー用出力デバイス")}
            <select
              value={previewDevice}
              onChange={(event) => setPreviewDevice(event.target.value)}
            >
              <option value="">{t("なし")}</option>
              {previewDevice && !outputDevices.includes(previewDevice) && (
                <option value={previewDevice}>
                  {previewDevice}
                  {t("（未検出）")}
                </option>
              )}
              {outputDevices.map((available) => (
                <option key={available}>{available}</option>
              ))}
            </select>
          </label>
        </>
      )}
    </>
  );
}
