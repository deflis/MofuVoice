import * as v from "valibot";
import { numberRange } from "../validation";

export const voiceSchema = v.object({
  pitchAlgorithm: v.picklist(["phase-locked", "pvsola", "td-psola", "fd-psola"]),
  pitch: numberRange(-12, 12, "ピッチ"),
  formantShift: numberRange(-12, 12, "フォルマント"),
  preserveFormants: v.boolean(),
});
export type VoiceValues = v.InferOutput<typeof voiceSchema>;
