import * as v from "valibot";
import { numberRange } from "../validation";

const thresholdDbMessage = "ゲートしきい値は-90〜0の数値を入力してください。";

export const noiseSchema = v.pipe(
  v.object({
    rumbleFilter: v.boolean(),
    enabled: v.boolean(),
    thresholdMode: v.picklist(["auto", "manual"]),
    // Manual range is enforced below; automatic mode ignores this value in Rust.
    thresholdDb: v.number(thresholdDbMessage),
    attackMs: numberRange(0.1, 2000, "ノイズゲートのアタック時間"),
    releaseMs: numberRange(0.1, 2000, "ノイズゲートのリリース時間"),
  }),
  v.check((noise) => {
    if (!Number.isFinite(noise.thresholdDb)) {
      return false;
    }
    // Automatic mode ignores thresholdDb in Rust; only manual mode needs the range.
    return noise.thresholdMode === "auto" || (noise.thresholdDb >= -90 && noise.thresholdDb <= 0);
  }, thresholdDbMessage),
);
export type NoiseValues = v.InferOutput<typeof noiseSchema>;
