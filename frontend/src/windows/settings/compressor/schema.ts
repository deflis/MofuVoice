import * as v from "valibot";
import { numberRange } from "../validation";
export const compressorDefaults = {
  enabled: false,
  thresholdDb: -12,
  ratio: 3,
  attackMs: 5,
  releaseMs: 150,
};
export const compressorSchema = v.object({
  enabled: v.boolean(),
  thresholdDb: numberRange(-36, 0, "しきい値（dBFS）"),
  ratio: numberRange(1, 20, "圧縮比"),
  attackMs: numberRange(0.1, 100, "アタック（ms）"),
  releaseMs: numberRange(10, 2000, "リリース（ms）"),
});
export type CompressorValues = v.InferOutput<typeof compressorSchema>;
