import * as v from "valibot";
import { numberRange } from "../validation";

export const eqBandSchema = v.object({
  frequency: numberRange(40, 16000, "EQ周波数"),
  gainDb: numberRange(-12, 12, "EQゲイン"),
  q: numberRange(0.2, 8, "EQのQ"),
});
export type EqBand = v.InferOutput<typeof eqBandSchema>;

export const eqSchema = v.object({
  enabled: v.boolean(),
  mode: v.picklist(["simple", "pro"]),
  bands: v.pipe(v.array(eqBandSchema), v.minLength(1, "EQには1つ以上のバンドが必要です。")),
  outputDb: numberRange(-18, 0, "EQ出力"),
});
export type EqValues = v.InferOutput<typeof eqSchema>;
