import * as v from "valibot";
import { numberRange } from "../validation";
export const limiterDefaults = { enabled: false, ceilingDb: -1, releaseMs: 80 };
export const limiterSchema = v.object({
  enabled: v.boolean(),
  ceilingDb: numberRange(-24, 0, "上限（dBFS）"),
  releaseMs: numberRange(1, 2000, "リリース（ms）"),
});
export type LimiterValues = v.InferOutput<typeof limiterSchema>;
