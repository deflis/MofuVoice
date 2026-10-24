import styles from "../settings.module.css";
import deviceStyles from "./devices.module.css";
import { NumberInput } from "../NumberInput";
import { useState } from "react";
import { useChannelRouting } from "./hooks";

export function ChannelRoutingPresets() {
  const { inputChannel, outputChannels, setRouting } = useChannelRouting();
  const singleOutput = inputChannel === 1 && outputChannels.length === 1 && outputChannels[0] === 1;
  const dualOutput =
    inputChannel === 1 &&
    outputChannels.length === 2 &&
    outputChannels.includes(1) &&
    outputChannels.includes(2);
  return (
    <fieldset className={deviceStyles["routing-presets"]}>
      <legend>チャンネル</legend>
      <div className={deviceStyles["routing-choices"]}>
        <button type="button" aria-pressed={singleOutput} onClick={() => setRouting(1, [1])}>
          モノラル（1 → 1）
        </button>
        <button type="button" aria-pressed={dualOutput} onClick={() => setRouting(1, [1, 2])}>
          ステレオ（1 → 1, 2）
        </button>
      </div>
      {!singleOutput && !dualOutput && (
        <p className={styles.muted}>詳細設定で指定したチャンネル割り当てを使用中です。</p>
      )}
    </fieldset>
  );
}

export function ChannelRoutingDetails() {
  const { inputChannel, outputChannels, setInputChannel, setOutputChannels } = useChannelRouting();
  const [extraChannel, setExtraChannel] = useState("");
  const additionalChannel = Number(extraChannel);

  const canAddChannel =
    Number.isInteger(additionalChannel) &&
    additionalChannel >= 1 &&
    additionalChannel <= 512 &&
    !outputChannels.includes(additionalChannel);
  const channelChoices = [...new Set([1, 2, 3, 4, 5, 6, 7, 8, ...outputChannels])];
  return (
    <>
      <p>
        チャンネル割り当て：入力{inputChannel} → 出力
        {outputChannels.length ? outputChannels.join("・") : "未選択"}
      </p>

      <label>
        入力チャンネル
        <NumberInput
          min="1"
          max="512"
          step="1"
          required
          value={inputChannel}
          onChange={(event) => setInputChannel(event.currentTarget.valueAsNumber)}
        />
      </label>
      <fieldset>
        <legend>出力チャンネル（複数選択可）</legend>
        <div className={deviceStyles["channel-choices"]}>
          {channelChoices.map((channel) => (
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
            placeholder="例: 9"
            value={extraChannel}
            onChange={(event) => setExtraChannel(event.target.value)}
          />
        </label>
        <button
          type="button"
          disabled={!canAddChannel}
          onClick={() => {
            setOutputChannels([...outputChannels, additionalChannel]);
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
    </>
  );
}
