export type LevelSettings = {
  enabled: boolean;
  targetDb: string;
  maxGainDb: string;
  floorDb: string;
  responseMs: string;
  thresholdDb: string;
  ratio: string;
  attackMs: string;
  releaseMs: string;
};
export type Settings = {
  level: LevelSettings;
  device: string;
  outputDevice: string;
  backend: string;
  frames: string;
  pitch: string;
  formantShift: string;
  preserveFormants: boolean;
  rumbleFilter: boolean;
  gate: boolean;
  gateThresholdMode: "auto" | "manual";
  gateThresholdDb: string;
  gateAttackMs: string;
  gateReleaseMs: string;
};
export type AudioInfo = {
  phase: "ready" | "opening" | "running" | "stopped" | "inspected" | "error";
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
export type Snapshot = { busy: boolean; info: Partial<AudioInfo> };
export type DeviceChoices = {
  devices: string[];
  outputDevices: string[];
  audioAvailable: boolean;
  deviceError: string;
  resolvedBackend: string;
};
export type Bootstrap = DeviceChoices & {
  settings: Settings;
  levelDefaults: LevelSettings;
  backends: string[];
  spectrumAvailable: boolean;
};
