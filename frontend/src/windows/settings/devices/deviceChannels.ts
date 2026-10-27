import type { DeviceSampleRates } from "../../../types";

export function channelLimits(
  layouts: DeviceSampleRates["channels"],
  sampleRate: number,
  inputChannel: number,
  outputChannels: number[],
) {
  const available = layouts.filter((layout) => layout.sampleRate === sampleRate);
  const requiredOutput = Math.max(1, ...outputChannels);
  return {
    available: available.length > 0,
    input: Math.max(
      0,
      ...available
        .filter((layout) => layout.output >= requiredOutput)
        .map((layout) => layout.input),
    ),
    output: Math.max(
      0,
      ...available.filter((layout) => layout.input >= inputChannel).map((layout) => layout.output),
    ),
  };
}
