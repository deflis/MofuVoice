import type { TFunction } from "i18next";

// Domain validation retains source-language messages in state. Convert the few
// interpolated legacy formats to i18next keys at the presentation boundary, so
// an already visible error follows language changes without changing validation.
export function translateError(message: string, t: TFunction): string {
  const range = /^(.*?)は(.*?)〜(.*?)の数値を入力してください。$/.exec(message);
  if (range) {
    return t("{0}は{1}〜{2}の数値を入力してください。", {
      v0: t(range[1]),
      v1: range[2],
      v2: range[3],
    });
  }
  const integer = /^(.*?)には整数を入力してください。$/.exec(message);
  if (integer) {
    return t("{0}には整数を入力してください。", { v0: t(integer[1]) });
  }
  const bands = /^EQバンドは(\d+)個までです。$/.exec(message);
  if (bands) {
    return t("EQバンドは{0}個までです。", { v0: bands[1] });
  }
  const state = /^不正な音声状態です（(.*?)）。$/.exec(message);
  if (state) {
    return t("不正な音声状態です（{0}）。", { v0: state[1] });
  }
  return t(message);
}
