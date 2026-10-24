import * as v from "valibot";
import { numberRange } from "../validation";
export const reverbDefaults = { enabled: false, mix: 0.15, preDelayMs: 20, decayMs: 1200 };
export const reverbSchema = v.object({
  enabled: v.boolean(),
  mix: numberRange(0, 1, "ウェット比率"),
  preDelayMs: numberRange(0, 200, "プリディレイ（ms）"),
  decayMs: numberRange(100, 10000, "減衰時間（ms・公称RT60）"),
});
export type ReverbValues = v.InferOutput<typeof reverbSchema>;
