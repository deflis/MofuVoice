import * as v from "valibot";
import { numberRange } from "../validation";

export const voiceSchema = v.object({
  pitchAlgorithm: v.picklist([
    "phase-locked",
    "pvsola",
    "pvsola-sync",
    "td-psola",
    "td-psola-resampled",
    "fd-psola",
  ]),
  psolaAnalysisWindow: v.picklist([1024, 2048, 4096, 8192]),
  psolaSynthesisWindow: v.picklist([256, 512, 1024, 2048]),
  analysisWindow: v.picklist([1024, 2048, 4096, 8192]),
  synthesisWindow: v.picklist([256, 512, 1024, 2048]),
  pitch: numberRange(-12, 12, "ピッチ"),
  formantShift: numberRange(-12, 12, "フォルマント"),
  preserveFormants: v.boolean(),
});
export type VoiceValues = v.InferOutput<typeof voiceSchema>;
