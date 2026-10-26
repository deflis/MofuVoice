export type { Settings, SettingsView } from "./windows/settings/schema";
export type { EqBand } from "./windows/settings/eq/schema";
export type { LevelValues as LevelSettings } from "./windows/settings/level/schema";
import type { DeesserValues } from "./windows/settings/deesser/schema";
export type DeEsserParameters = Omit<DeesserValues, "enabled" | "mode">;
export type MainSetup = {
  audioAvailable: boolean;
  resolvedBackend: string;
  inputConfigured: boolean;
  outputConfigured: boolean;
  rememberedDeviceMissing: boolean;
};
export type AudioPhase = {
  phase: string;
  error: string | null;
  busy: boolean;
};
export type MainView = {
  canStart: boolean;
  spectrumAvailable: boolean;
  setup: MainSetup;
  phase: AudioPhase;
};
export type AudioInfo = {
  phase: string;
  error: string | null;
  device: string | null;
  outputDevice: string | null;
  backend: string | null;
  sampleRate: number | null;
  inputs: number | null;
  outputs: number | null;
  frames: number | null;
  periodMs: number | null;
  dspMs: number | null;
  inputLatencyMs: number | null;
  outputLatencyMs: number | null;
};
export type AudioLog = {
  elapsedSeconds: number | null;
  telemetry: {
    blocks: number;
    nonzeroSamples: number;
    maxCallbackUs: number;
    deadlineMisses: number;
    timestampBlocks: number | null;
    samplePositionGaps: number | null;
    faultFlags: number;
    outputBlocks: number | null;
    underruns: number | null;
    overruns: number | null;
    queueFrames: number | null;
    observedInputFrames: number | null;
    observedOutputFrames: number | null;
    queuePeakFrames: number | null;
    startupQueueFrames: number | null;
    queueCapacityFrames: number | null;
    maxInputCallbackUs: number | null;
    maxOutputCallbackUs: number | null;
  } | null;
};
export type DeviceChoices = {
  devices: string[];
  outputDevices: string[];
  audioAvailable: boolean;
  deviceError: string;
  resolvedBackend: string;
};
export type DeviceSampleRates = {
  input: number[];
  output: number[];
  common: number[];
  channels: { sampleRate: number; input: number; output: number }[];
};
