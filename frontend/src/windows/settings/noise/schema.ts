import * as v from "valibot";
import { numberRange } from "../validation";

export const noiseSchema = v.object({
  rumbleFilter: v.boolean(),
  enabled: v.boolean(),
  thresholdMode: v.picklist(["auto", "manual"]),
  thresholdDb: numberRange(-90, 0, "ゲートしきい値"),
  attackMs: numberRange(0.1, 2000, "ゲートアタック"),
  releaseMs: numberRange(0.1, 2000, "ゲートリリース"),
});
export type NoiseValues = v.InferOutput<typeof noiseSchema>;
