/** src/level.rs::reduction: compressor alone, before its shared peak guard. */
export function compressorOutputDb(input: number, threshold: number, ratio: number) {
  const over = input - threshold;
  const slope = 1 - 1 / ratio;
  let reduction = 0;
  if (over > -3 && over < 3) {
    reduction = (slope * (over + 3) ** 2) / 12;
  } else if (over >= 3) {
    reduction = slope * over;
  }
  return input - reduction;
}
