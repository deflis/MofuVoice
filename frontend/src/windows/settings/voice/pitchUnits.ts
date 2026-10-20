export function semitonesToPercent(semitones: number): number {
  return 100 * 2 ** (semitones / 12);
}

export function percentToSemitones(percent: number): number {
  return Number.isFinite(percent) && percent > 0 ? 12 * Math.log2(percent / 100) : NaN;
}
