import { useSettingsLocked } from "../settingsHooks";
import { DeviceSettings } from "./DeviceSettings";
import { VoiceSettings } from "./VoiceSettings";
import { NoiseGateSettings } from "./NoiseGateSettings";
import { AutoLevelSettings } from "./AutoLevelSettings";

export function SettingsPanel() {
  const locked = useSettingsLocked();
  return (
    <fieldset className="settings" disabled={locked}>
      <div className="grid">
        <DeviceSettings />
        <VoiceSettings />
      </div>
      <NoiseGateSettings />
      <AutoLevelSettings />
    </fieldset>
  );
}
