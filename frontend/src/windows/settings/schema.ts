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
    (settings) => settings.voice.synthesisWindow <= settings.voice.analysisWindow,
    "合成窓は解析窓以下にしてください。",
  ),
  v.check(
    (settings) => settings.voice.preserveFormants || settings.voice.formantShift === 0,
    "フォルマントを独立調整しない場合、調整値は0にしてください。",
  ),
  v.check(
    (settings) =>
      !settings.voice.pitchAlgorithm.startsWith("td-psola") || settings.voice.preserveFormants,
    "TD-PSOLAではフォルマント保持をオンにしてください。",
  ),
  v.check(
    (settings) => settings.level.floorDb < settings.level.targetDb,
    "音声検出しきい値は目標音量より低くしてください。",
  ),
  v.check(
    (settings) => settings.devices.backend !== "asio" || settings.devices.outputDevice === "",
    "ASIOでは入出力に同じデバイスを使います。",
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
