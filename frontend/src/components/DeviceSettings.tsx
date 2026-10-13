import { useState } from "react";
import { useDeviceSettings } from "../settingsHooks";

export function DeviceSettings() {
  const [extraChannel, setExtraChannel] = useState("");
  const additionalChannel = Number(extraChannel);

  const {
    device,
    outputDevice,
    inputChannel,
    outputChannels,
    backend,
    resolvedBackend,
    frames,
    devices,
    outputDevices,
    backends,
    update,
    updateBackend,
  } = useDeviceSettings();
  const canAddChannel =
    Number.isInteger(additionalChannel) &&
    additionalChannel >= 1 &&
    additionalChannel <= 512 &&
    !outputChannels.includes(additionalChannel);
  const channelChoices = [...new Set([1, 2, 3, 4, 5, 6, 7, 8, ...outputChannels])];
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
        入力チャンネル
        <input
          type="number"
          min="1"
          max="512"
          step="1"
          required
          value={inputChannel}
          onChange={(event) => update("inputChannel", Number(event.target.value))}
        />
      </label>
      <fieldset>
        <legend>出力チャンネル（複数選択可）</legend>
        <div className="channel-choices">
          {channelChoices.map((channel) => (
            <label className="check" key={channel}>
              <input
                type="checkbox"
                checked={outputChannels.includes(channel)}
                onChange={(event) =>
                  update(
                    "outputChannels",
                    event.target.checked
                      ? [...outputChannels, channel]
                      : outputChannels.filter((selected) => selected !== channel),
                  )
                }
              />
              {channel}ch
            </label>
          ))}
        </div>
        <label>
          追加する出力チャンネル
          <input
            type="number"
            min="1"
            max="512"
            step="1"
            placeholder="9以上"
            value={extraChannel}
            onChange={(event) => setExtraChannel(event.target.value)}
          />
        </label>
        <button
          type="button"
          disabled={!canAddChannel}
          onClick={() => {
            update("outputChannels", [...outputChannels, additionalChannel]);
            setExtraChannel("");
          }}
        >
          追加して選択
        </button>
        <p>選択したすべての出力へ同じ音声を送ります。変更は次の開始時に反映されます。</p>
        {outputChannels.length === 0 && (
          <p role="alert">出力チャンネルを1つ以上選択してください。</p>
        )}
      </fieldset>
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
