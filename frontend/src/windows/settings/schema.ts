import { compressorSchema } from "./compressor/schema";
import { limiterSchema } from "./limiter/schema";
import { dereverbSchema } from "./dereverb/schema";
import { reverbSchema } from "./reverb/schema";
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
    inputEq: eqSchema,
    preConversionEq: eqSchema,
    compressor: compressorSchema,
    limiter: limiterSchema,
    dereverb: dereverbSchema,
    reverb: reverbSchema,

    level: levelSchema,
  }),
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
  chainStages: v.array(v.string()),
});
export type SettingsView = v.InferOutput<typeof settingsViewSchema>;
