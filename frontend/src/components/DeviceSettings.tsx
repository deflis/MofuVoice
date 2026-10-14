import { DetailsPopup } from "./DetailsPopup";
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
    updateRouting,
  } = useDeviceSettings();
  const canAddChannel =
    Number.isInteger(additionalChannel) &&
    additionalChannel >= 1 &&
    additionalChannel <= 512 &&
    !outputChannels.includes(additionalChannel);
  const singleOutput = inputChannel === 1 && outputChannels.length === 1 && outputChannels[0] === 1;
  const dualOutput =
    inputChannel === 1 &&
    outputChannels.length === 2 &&
    outputChannels.includes(1) &&
    outputChannels.includes(2);
  const channelChoices = [...new Set([1, 2, 3, 4, 5, 6, 7, 8, ...outputChannels])];
  return (
    <section>
      <h2>音声デバイス</h2>
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
      <fieldset className="routing-presets">
        <legend>チャンネル</legend>
        <div className="routing-choices">
          <button type="button" aria-pressed={singleOutput} onClick={() => updateRouting(1, [1])}>
            モノラル（1 → 1）
          </button>
          <button type="button" aria-pressed={dualOutput} onClick={() => updateRouting(1, [1, 2])}>
            ステレオ（1 → 1, 2）
          </button>
        </div>
        {!singleOutput && !dualOutput && (
          <p className="muted">プロ向け設定の割り当てを使用中です。</p>
        )}
      </fieldset>
      <DetailsPopup title="音声デバイス" buttonLabel="プロ向け設定">
        <fieldset className="settings">
          <legend>音声バックエンド</legend>
          <div className="routing-choices">
            {backends.map((available) => (
              <button
                key={available}
                type="button"
                aria-pressed={backend === available}
                onClick={() => void updateBackend(available)}
              >
                {{ auto: "自動", cpal: "標準", asio: "ASIO" }[available] ?? available}
              </button>
            ))}
          </div>
          {!backends.includes(backend) && (
            <p role="alert">現在の接続方式「{backend}」は利用できません。選び直してください。</p>
          )}
        </fieldset>
        <p>
          チャンネル割り当て：入力{inputChannel} → 出力
          {outputChannels.length ? outputChannels.join("・") : "未選択"}
        </p>

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
          バッファサイズ（サンプル）
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
        <p>
          小さくすると遅延が減ります。音切れする場合は大きくしてください。デバイスによっては指定値が調整されます。
        </p>
      </DetailsPopup>
    </section>
  );
}
