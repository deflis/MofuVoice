import type { VoiceValues } from "./schema";

/** Buffered wet-path delay in samples; device and neutral bypass delays are separate. */
export function pitchLatency(
  algorithm: VoiceValues["pitchAlgorithm"],
  vocoderSynthesis: VoiceValues["synthesisWindow"],
  analysis: VoiceValues["psolaAnalysisWindow"],
  synthesis: VoiceValues["psolaSynthesisWindow"],
  formantShift: number,
): number {
  if (algorithm === "pvsola") {
    return 4608;
  }
  if (algorithm === "phase-locked" || algorithm === "pvsola-sync") {
    return vocoderSynthesis;
  }
  const left = Math.ceil(3200 / (1 + Math.sqrt(synthesis / analysis)));
  const latency =
    Math.ceil((analysis / 2 + 400 + 16 + 1600 + Math.max(left, synthesis / 2) + 128 + 66) / 128) *
    128;
  return (
    latency + (algorithm !== "td-psola-resampled" && formantShift !== 0 ? vocoderSynthesis : 0)
  );
}
