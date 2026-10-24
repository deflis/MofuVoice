import styles from "../settings.module.css";
import deviceStyles from "./devices.module.css";
import { NumberInput } from "../NumberInput";
import { DetailsPopup } from "../DetailsPopup";
import { ChannelRoutingDetails } from "./ChannelRoutingSettings";
import { useBackendSettings, useBufferSettings } from "./hooks";

function BackendSettings() {
  const { backend, backends, changeBackend } = useBackendSettings();
  return (
    <fieldset className={styles.settings}>
      <legend>音声バックエンド</legend>
      <div className={deviceStyles["routing-choices"]}>
        {backends.map((available) => (
          <button
            key={available}
            type="button"
            aria-pressed={backend === available}
            onClick={() => void changeBackend(available)}
          >
            {{ auto: "自動", cpal: "標準", asio: "ASIO" }[available] ?? available}
          </button>
        ))}
      </div>
      {!backends.includes(backend) && (
        <p role="alert">現在の接続方式「{backend}」は利用できません。選び直してください。</p>
      )}
    </fieldset>
  );
}

function BufferSettings() {
  const { frames, setFrames } = useBufferSettings();
  return (
    <>
      <label>
        バッファサイズ（サンプル）
        <NumberInput
          min="1"
          max="8192"
          step="1"
          required
          value={frames}
          onChange={(event) => setFrames(event.currentTarget.valueAsNumber)}
        />
      </label>
      <p>
        小さくすると遅延が減ります。音切れする場合は大きくしてください。デバイスによっては指定値が調整されます。
      </p>
    </>
  );
}

export function DeviceAdvancedSettings() {
  return (
    <DetailsPopup title="音声デバイス" buttonLabel="詳細設定">
      <BackendSettings />
      <ChannelRoutingDetails />
      <BufferSettings />
    </DetailsPopup>
  );
}
