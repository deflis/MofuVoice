import * as v from "valibot";
import { numberRange } from "../validation";

export const levelSchema = v.object({
  enabled: v.boolean(),
  compressorEnabled: v.boolean(),
  targetDb: numberRange(-36, -6, "目標音量"),
  maxGainDb: numberRange(0, 18, "最大増幅量"),
  floorDb: numberRange(-90, -30, "音声検出しきい値"),
  responseMs: numberRange(100, 10000, "音量調整の応答時間"),
  thresholdDb: numberRange(-36, 0, "圧縮しきい値"),
  ratio: numberRange(1, 20, "圧縮比"),
  attackMs: numberRange(0.1, 100, "コンプレッサーのアタック時間"),
  releaseMs: numberRange(10, 2000, "コンプレッサーのリリース時間"),
});
export type LevelValues = v.InferOutput<typeof levelSchema>;
