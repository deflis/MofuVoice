import * as v from "valibot";
import { numberRange } from "../validation";

export const deesserSchema = v.object({
  enabled: v.boolean(),
  mode: v.picklist(["simple", "pro"]),
  frequencyHz: numberRange(2000, 12000, "ディエッサー周波数"),
  thresholdDb: numberRange(-60, 0, "ディエッサーしきい値"),
  rangeDb: numberRange(0, 12, "ディエッサー減衰量"),
  attackMs: numberRange(0.1, 20, "ディエッサーアタック"),
  releaseMs: numberRange(10, 500, "ディエッサーリリース"),
});
export type DeesserValues = v.InferOutput<typeof deesserSchema>;
