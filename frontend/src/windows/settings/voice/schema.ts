import * as v from "valibot";
import { numberRange } from "../validation";

export const voiceSchema = v.pipe(
  v.object({
    enabled: v.optional(v.boolean(), true),
    pitchUnit: v.optional(v.picklist(["semitones", "percent"]), "semitones"),
    pitchAlgorithm: v.picklist([
      "phase-locked",
      "pvsola",
      "pvsola-sync",
      "td-psola",
      "td-psola-resampled",
      "fd-psola",
    ]),
    mathPrecision: v.picklist(["standard", "accurate"]),
    psolaPitchEstimator: v.optional(v.picklist(["yin", "mpm", "mpm-fft", "nccf"]), "yin"),
    psolaAnalysisWindow: v.picklist([1024, 2048, 4096, 8192]),
    psolaSynthesisWindow: v.picklist([256, 512, 1024, 2048]),
    analysisWindow: v.picklist([1024, 2048, 4096, 8192]),
    synthesisWindow: v.picklist([256, 512, 1024, 2048]),
    pitch: numberRange(-12, 12, "ピッチ"),
    formantShift: numberRange(-12, 12, "フォルマント"),
    preserveFormants: v.boolean(),
  }),
  v.check(
    (voice) => voice.psolaSynthesisWindow <= voice.psolaAnalysisWindow,
    "PSOLAの合成ウィンドウ長は解析ウィンドウ長以下にしてください。",
  ),
  v.check(
    (voice) => voice.synthesisWindow <= voice.analysisWindow,
    "合成ウィンドウ長は解析ウィンドウ長以下にしてください。",
  ),
  v.check(
    (voice) => voice.preserveFormants || voice.formantShift === 0,
    "「フォルマントを独立して調整する」が無効の場合、調整値は0にしてください。",
  ),
  v.check(
    (voice) => !voice.pitchAlgorithm.startsWith("td-psola") || voice.preserveFormants,
    "TD-PSOLAでは「フォルマントを独立して調整する」をオンにしてください。",
  ),
);
export type VoiceValues = v.InferOutput<typeof voiceSchema>;
