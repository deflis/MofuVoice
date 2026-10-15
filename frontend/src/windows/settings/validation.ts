import * as v from "valibot";

export function numberRange(min: number, max: number, label: string) {
  const message = `${label}は${min}〜${max}の数値を入力してください。`;
  return v.pipe(
    v.number(message),
    v.finite(message),
    v.minValue(min, message),
    v.maxValue(max, message),
  );
}
export function integerRange(min: number, max: number, label: string) {
  return v.pipe(numberRange(min, max, label), v.integer(`${label}には整数を入力してください。`));
}
