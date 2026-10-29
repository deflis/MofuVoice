import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import type { RateResult } from "./deviceSampleRates";
import { useSampleRateSettings } from "./hooks";
import { useSettingsLocked } from "../hooks";

function ratesText(rates: number[], t: ReturnType<typeof useTranslation>["t"]) {
  return rates.length ? rates.map((rate) => `${rate.toLocaleString()} Hz`).join(", ") : t("なし");
}
function SupportedRates({
  result,
  backend,
  sampleRate,
  locked,
  device,
}: {
  result: RateResult | undefined;
  backend: string;
  sampleRate: number;
  locked: boolean;
  device: string;
}) {
  const { t } = useTranslation();
  if (result?.rates) {
    return (
      <>
        {backend === "cpal" && (
          <>
            <p>
              {t("入力の対応レート：")}
              {ratesText(result.rates.input, t)}
            </p>
            <p>
              {t("出力の対応レート：")}
              {ratesText(result.rates.output, t)}
            </p>
          </>
        )}
        <p>
          {backend === "asio" ? t("デバイスの対応レート") : t("入出力で変換なしに使えるレート")}：
          {ratesText(result.rates.common, t)}
        </p>
        {!result.rates.common.includes(sampleRate) && (
          <p role="alert">
            {t("選択中の")}
            {sampleRate.toLocaleString()} Hz
            {backend === "cpal"
              ? t(
                  "での直接入出力に対応していないため、デバイスの対応レートとの間でリサンプリングします。",
                )
              : t("は対応していません。対応レートを選び直してください。")}
          </p>
        )}
        <p className={styles.muted}>
          {t(
            "アプリで選択できるレートの対応状況です。チャンネルとバッファサイズは開始時に確認します。",
          )}
        </p>
      </>
    );
  }
  if (result?.error) {
    return (
      <p role="alert">
        {t("対応レートを取得できませんでした：")}
        {result.error}
      </p>
    );
  }
  let message = t("対応レートを確認中…");
  if (locked) {
    message = t("音声を停止すると対応レートを確認できます。");
  } else if (backend === "asio" && !device) {
    message = t("デバイスを選ぶと対応レートを表示します。");
  } else if (!backend) {
    message = t("音声デバイスを確認中…");
  }
  return <p className={styles.muted}>{message}</p>;
}

export function SampleRateSettings({ result }: { result: RateResult | undefined }) {
  const { t } = useTranslation();
  const { device, resolvedBackend, sampleRate, setSampleRate } = useSampleRateSettings();
  const locked = useSettingsLocked();
  return (
    <>
      <label>
        {t("サンプリングレート")}
        <select value={sampleRate} onChange={(event) => setSampleRate(Number(event.target.value))}>
          {[44100, 48000].map((rate) => (
            <option key={rate} value={String(rate)}>
              {rate.toLocaleString()} Hz
            </option>
          ))}
        </select>
      </label>
      <div aria-live="polite">
        <SupportedRates
          result={result}
          backend={resolvedBackend}
          sampleRate={sampleRate}
          locked={locked}
          device={device}
        />
      </div>
      <p className={styles.muted}>
        {t(
          "CPALでは選択レートに非対応の入出力を自動でリサンプリングします。変換により遅延が追加されます。変更は次の開始時に反映されます。ASIOではドライバー側の設定も同じレートに揃えてください。",
        )}
      </p>
    </>
  );
}
