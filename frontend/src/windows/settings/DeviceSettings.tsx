import { DeviceSelection } from "./DeviceSelection";
import { SampleRateSettings } from "./SampleRateSettings";
import { ChannelRoutingPresets } from "./ChannelRoutingSettings";
import { DeviceAdvancedSettings } from "./DeviceAdvancedSettings";

export function DeviceSettings() {
  return (
    <section>
      <h2>音声デバイス</h2>
      <DeviceSelection />
      <SampleRateSettings />
      <ChannelRoutingPresets />
      <DeviceAdvancedSettings />
    </section>
  );
}
