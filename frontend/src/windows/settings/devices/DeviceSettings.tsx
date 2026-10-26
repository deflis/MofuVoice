import { DeviceSelection } from "./DeviceSelection";
import { SampleRateSettings } from "./SampleRateSettings";
import { ChannelRoutingDetails } from "./ChannelRoutingSettings";
import { DeviceAdvancedSettings } from "./DeviceAdvancedSettings";
import { useSampleRateSettings } from "./hooks";
import { useSettingsLocked } from "../hooks";
import { useDeviceSampleRates } from "./useDeviceSampleRates";

export function DeviceSettings() {
  const { device, outputDevice, resolvedBackend } = useSampleRateSettings();
  const locked = useSettingsLocked();
  const result = useDeviceSampleRates({ backend: resolvedBackend, device, outputDevice }, locked);
  return (
    <section>
      <h2>音声デバイス</h2>
      <DeviceSelection />
      <SampleRateSettings result={result} />
      <ChannelRoutingDetails result={result} />
      <DeviceAdvancedSettings />
    </section>
  );
}
