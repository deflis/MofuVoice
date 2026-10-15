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
export type DeEsserParameters = {
  frequencyHz: string;
  thresholdDb: string;
  rangeDb: string;
  attackMs: string;
  releaseMs: string;
};
export type EqBand = { frequency: string; gainDb: string; q: string };
export type EqSettings = {
  enabled: boolean;
  mode: "simple" | "pro";
  bands: [EqBand, EqBand, EqBand];
  outputDb: string;
};
export type Settings = {
  eq: EqSettings;
  level: LevelSettings;
  device: string;
  outputDevice: string;
  inputChannel: number;
  outputChannels: number[];
  backend: string;
  frames: string;
  pitch: string;
  formantShift: string;
  preserveFormants: boolean;
  rumbleFilter: boolean;
  deesser: boolean;
  deesserMode: "simple" | "pro";
  deesserParameters: DeEsserParameters;
  gate: boolean;
  gateThresholdMode: "auto" | "manual";
  gateThresholdDb: string;
  gateAttackMs: string;
  gateReleaseMs: string;
};
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
export type SettingsView = DeviceChoices & {
  settings: Settings;
  levelDefaults: LevelSettings;
  backends: string[];
  audioBusy: boolean;
};
