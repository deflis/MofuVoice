import * as v from "valibot";
import { integerRange } from "../validation";

export const devicesSchema = v.object({
  backend: v.picklist(["auto", "cpal", "asio"]),
  device: v.string(),
  outputDevice: v.string(),
  inputChannel: integerRange(1, 512, "入力チャンネル"),
  outputChannels: v.pipe(
    v.array(integerRange(1, 512, "出力チャンネル")),
    v.minLength(1, "出力チャンネルを1つ以上選択してください。"),
    v.check(
      (channels) => new Set(channels).size === channels.length,
      "出力チャンネルが重複しています。",
    ),
  ),
  frames: integerRange(1, 8192, "バッファサイズ"),
  sampleRate: v.pipe(
    v.number(),
    v.check(
      (rate) => rate === 44100 || rate === 48000,
      "対応するサンプリングレートを選択してください。",
    ),
  ),
});
export type DevicesValues = v.InferOutput<typeof devicesSchema>;
