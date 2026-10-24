import * as v from "valibot";
import { numberRange } from "../validation";
export const dereverbDefaults = { enabled: false, amount: 0.5, decayMs: 250 };
export const dereverbSchema = v.object({
  enabled: v.boolean(),
  amount: numberRange(0, 1, "抑制量"),
  decayMs: numberRange(20, 2000, "減衰時間（ms）"),
});
export type DereverbValues = v.InferOutput<typeof dereverbSchema>;
