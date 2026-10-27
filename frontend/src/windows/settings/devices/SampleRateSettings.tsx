import styles from "../settings.module.css";
import type { RateResult } from "./deviceSampleRates";
import { useSampleRateSettings } from "./hooks";
import { useSettingsLocked } from "../hooks";

function ratesText(rates: number[]) {
  return rates.length ? rates.map((rate) => `${rate.toLocaleString()} Hz`).join("、") : "なし";
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
  if (result?.rates) {
    return (
      <>
        {backend === "cpal" && (
          <>
            <p>入力の対応レート：{ratesText(result.rates.input)}</p>
            <p>出力の対応レート：{ratesText(result.rates.output)}</p>
          </>
        )}
        <p>
          {backend === "asio" ? "デバイスの対応レート" : "入出力で変換なしに使えるレート"}：
          {ratesText(result.rates.common)}
        </p>
        {!result.rates.common.includes(sampleRate) && (
          <p role="alert">
            選択中の {sampleRate.toLocaleString()} Hz
            {backend === "cpal"
              ? "での直接入出力に対応していないため、デバイスの対応レートとの間でリサンプリングします。"
              : "は対応していません。対応レートを選び直してください。"}
          </p>
        )}
        <p className={styles.muted}>
          アプリで選択できるレートの対応状況です。チャンネルとバッファサイズは開始時に確認します。
        </p>
      </>
    );
  }
  if (result?.error) {
    return <p role="alert">対応レートを取得できませんでした：{result.error}</p>;
  }
  let message = "対応レートを確認中…";
  if (locked) {
    message = "音声を停止すると対応レートを確認できます。";
  } else if (backend === "asio" && !device) {
    message = "デバイスを選ぶと対応レートを表示します。";
  } else if (!backend) {
    message = "音声デバイスを確認中…";
  }
  return <p className={styles.muted}>{message}</p>;
}

export function SampleRateSettings({ result }: { result: RateResult | undefined }) {
  const { device, resolvedBackend, sampleRate, setSampleRate } = useSampleRateSettings();
  const locked = useSettingsLocked();
  return (
    <>
      <label>
        サンプリングレート
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
        CPALでは選択レートに非対応の入出力を自動でリサンプリングします。変換により遅延が追加されます。変更は次の開始時に反映されます。ASIOではドライバー側の設定も同じレートに揃えてください。
      </p>
    </>
  );
}
