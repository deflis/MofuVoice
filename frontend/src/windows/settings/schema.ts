import * as v from "valibot";
import { devicesSchema } from "./devices/schema";
import { voiceSchema } from "./voice/schema";
import { noiseSchema } from "./noise/schema";
import { deesserSchema } from "./deesser/schema";
import { eqSchema } from "./eq/schema";
import { levelSchema } from "./level/schema";

// Object schemas deliberately extract only values from the store, excluding setters.
export const settingsSchema = v.pipe(
  v.object({
    devices: devicesSchema,
    voice: voiceSchema,
    noise: noiseSchema,
    deesser: deesserSchema,
    eq: eqSchema,
    level: levelSchema,
  }),
  v.check(
    (settings) => settings.voice.psolaSynthesisWindow <= settings.voice.psolaAnalysisWindow,
    "PSOLAの合成ウィンドウ長は解析ウィンドウ長以下にしてください。",
  ),
  v.check(
    (settings) => settings.voice.synthesisWindow <= settings.voice.analysisWindow,
    "合成ウィンドウ長は解析ウィンドウ長以下にしてください。",
  ),
  v.check(
    (settings) => settings.voice.preserveFormants || settings.voice.formantShift === 0,
    "「フォルマントを独立して調整する」が無効の場合、調整値は0にしてください。",
  ),
  v.check(
    (settings) =>
      !settings.voice.pitchAlgorithm.startsWith("td-psola") || settings.voice.preserveFormants,
    "TD-PSOLAでは「フォルマントを独立して調整する」をオンにしてください。",
  ),
  v.check(
    (settings) => settings.level.floorDb < settings.level.targetDb,
    "音声検出しきい値は目標音量より低くしてください。",
  ),
  v.check(
    (settings) => settings.devices.backend !== "asio" || settings.devices.outputDevice === "",
    "ASIO使用時は入力と出力で個別のデバイスを指定できません。同一デバイスを選択するか、出力デバイスの指定を解除してください。",
  ),
);
export type Settings = v.InferOutput<typeof settingsSchema>;
export const deviceChoicesSchema = v.object({
  devices: v.array(v.string()),
  outputDevices: v.array(v.string()),
  audioAvailable: v.boolean(),
  deviceError: v.string(),
  resolvedBackend: v.picklist(["asio", "cpal"]),
});
export const settingsViewSchema = v.object({
  ...deviceChoicesSchema.entries,
  settings: settingsSchema,
  levelDefaults: levelSchema,
  backends: v.array(v.string()),
});
export type SettingsView = v.InferOutput<typeof settingsViewSchema>;
