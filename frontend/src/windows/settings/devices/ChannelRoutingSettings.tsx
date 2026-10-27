import styles from "../settings.module.css";
import deviceStyles from "./devices.module.css";
import { useChannelRouting, useSampleRateSettings } from "./hooks";
import type { RateResult } from "./deviceSampleRates";
import { channelLimits } from "./deviceChannels";

export function ChannelRoutingDetails({ result }: { result: RateResult | undefined }) {
  const { inputChannel, outputChannels, setInputChannel, setOutputChannels } = useChannelRouting();
  const { sampleRate } = useSampleRateSettings();
  const channels =
    result?.rates && channelLimits(result.rates.channels, sampleRate, inputChannel, outputChannels);
  const inputChoices = Array.from({ length: channels?.input ?? 0 }, (_, index) => index + 1);
  const outputChoices = channels
    ? [
        ...new Set([
          ...Array.from({ length: channels.output }, (_, index) => index + 1),
          ...outputChannels,
        ]),
      ]
    : [];
  return (
    <>
      <p>
        チャンネル割り当て：入力{inputChannel} → 出力
        {outputChannels.length ? outputChannels.join("・") : "未選択"}
      </p>

      <fieldset>
        <legend>入力チャンネル（1つ選択）</legend>
        <div className={deviceStyles["routing-choices"]}>
          {inputChoices.map((channel) => (
            <button
              key={channel}
              type="button"
              aria-pressed={inputChannel === channel}
              onClick={() => setInputChannel(channel)}
            >
              {channel}ch
            </button>
          ))}
        </div>
      </fieldset>
      {!result && <p>利用可能なチャンネルを確認中です。音声動作中は停止して確認してください。</p>}
      {result?.error && <p role="alert">チャンネル情報を取得できません：{result.error}</p>}
      {result?.rates && !channels?.available && (
        <p role="alert">このサンプリングレートで利用できる入出力チャンネルがありません。</p>
      )}
      {channels &&
        (inputChannel > channels.input ||
          outputChannels.some((channel) => channel > channels.output)) && (
          <p role="alert">選択中のチャンネルはこのデバイスで利用できません。選び直してください。</p>
        )}
      <fieldset>
        <legend>出力チャンネル（複数選択可）</legend>
        <div className={deviceStyles["channel-choices"]}>
          {outputChoices.map((channel) => (
            <label className={styles.check} key={channel}>
              <input
                type="checkbox"
                checked={outputChannels.includes(channel)}
                onChange={(event) =>
                  setOutputChannels(
                    event.target.checked
                      ? [...outputChannels, channel]
                      : outputChannels.filter((selected) => selected !== channel),
                  )
                }
              />
              {channel}ch {channels && channel > channels.output ? "（利用不可）" : ""}
            </label>
          ))}
        </div>
        <p>選択したすべての出力へ同じ音声を送ります。変更は次の開始時に反映されます。</p>
        {outputChannels.length === 0 && (
          <p role="alert">出力チャンネルを1つ以上選択してください。</p>
        )}
      </fieldset>
    </>
  );
}
