import type { EqValues } from "../eq/schema";

/** Bilinear-transform 80 Hz Butterworth high-pass from src/rumble.rs. */
export function rumbleResponseDb(rate: number, frequency: number): number {
  if (
    ![44100, 48000].includes(rate) ||
    !Number.isFinite(frequency) ||
    frequency <= 0 ||
    frequency >= rate / 2
  ) {
    return NaN;
  }
  const ratio = Math.tan((Math.PI * frequency) / rate) / Math.tan((Math.PI * 80) / rate);
  return -10 * Math.log10(1 + ratio ** -4);
}

/** Small-signal response of src/eq.rs, including its reserved positive-gain headroom.
 * enabled deliberately does not affect this preview of retained settings. */
export function eqResponseDb(
  settings: Pick<EqValues, "bands" | "outputDb">,
  rate: number,
  frequency: number,
): number {
  if (
    ![44100, 48000].includes(rate) ||
    !Number.isFinite(frequency) ||
    frequency < 0 ||
    frequency > rate / 2 ||
    !Number.isFinite(settings.outputDb) ||
    settings.outputDb < -18 ||
    settings.outputDb > 0 ||
    settings.bands.length < 1 ||
    settings.bands.length > 16
  ) {
    return NaN;
  }
  let db = settings.outputDb;
  const phase = (2 * Math.PI * frequency) / rate;
  const c1 = Math.cos(phase),
    s1 = Math.sin(phase);
  const c2 = Math.cos(2 * phase),
    s2 = Math.sin(2 * phase);
  for (const band of settings.bands) {
    if (
      ![band.frequency, band.gainDb, band.q].every(Number.isFinite) ||
      band.frequency < 40 ||
      band.frequency > 16000 ||
      band.gainDb < -12 ||
      band.gainDb > 12 ||
      band.q < 0.2 ||
      band.q > 8
    ) {
      return NaN;
    }
    db -= Math.max(0, band.gainDb);
    if (band.gainDb === 0) {
      continue;
    }
    const a = 10 ** (band.gainDb / 40);
    const omega = (2 * Math.PI * band.frequency) / rate;
    const alpha = Math.sin(omega) / (2 * band.q);
    const norm = 1 / (1 + alpha / a);
    const b0 = (1 + alpha * a) * norm;
    const b1 = -2 * Math.cos(omega) * norm;
    const b2 = (1 - alpha * a) * norm;
    const a1 = b1,
      a2 = (1 - alpha / a) * norm;
    const numerator = (b0 + b1 * c1 + b2 * c2) ** 2 + (b1 * s1 + b2 * s2) ** 2;
    const denominator = (1 + a1 * c1 + a2 * c2) ** 2 + (a1 * s1 + a2 * s2) ** 2;
    db += 10 * Math.log10(numerator / denominator);
  }
  return db;
}
