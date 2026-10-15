import type { EqValues } from "./schema";
export function copyEqBands(bands: EqValues["bands"]): EqValues["bands"] {
  return bands.map((band) => ({ ...band }));
}
