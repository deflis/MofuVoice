import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import deviceStyles from "./devices.module.css";
import { NumberInput } from "../NumberInput";
import { DetailsPopup } from "../DetailsPopup";

import { useBackendSettings, useBufferSettings } from "./hooks";

function BackendSettings() {
  const { t } = useTranslation();
  const { backend, backends, changeBackend } = useBackendSettings();
  return (
    <fieldset className={styles.settings}>
      <legend>{t("音声バックエンド")}</legend>
      <div className={deviceStyles["routing-choices"]}>
        {backends.map((available) => (
          <button
            key={available}
            type="button"
            aria-pressed={backend === available}
            onClick={() => void changeBackend(available)}
          >
            {{ auto: t("自動"), cpal: t("標準"), asio: "ASIO" }[available] ?? available}
          </button>
        ))}
      </div>
      {!backends.includes(backend) && (
        <p role="alert">
          {t("現在の接続方式「")}
          {backend}
          {t("」は利用できません。選び直してください。")}
        </p>
      )}
    </fieldset>
  );
}

function BufferSettings() {
  const { t } = useTranslation();
  const { frames, setFrames } = useBufferSettings();
  return (
    <>
      <label>
        {t("バッファサイズ（サンプル）")}
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
        {t(
          "小さくすると遅延が減ります。音切れする場合は大きくしてください。デバイスによっては指定値が調整されます。",
        )}
      </p>
    </>
  );
}

export function DeviceAdvancedSettings() {
  const { t } = useTranslation();
  return (
    <DetailsPopup title={t("音声デバイス")} buttonLabel={t("詳細設定")}>
      <BackendSettings />
      <BufferSettings />
    </DetailsPopup>
  );
}
