import { useSettingsLocked } from "../settingsHooks";
import { DeviceSettings } from "./DeviceSettings";
import { DeEsserSettings } from "./DeEsserSettings";
import { VoiceSettings } from "./VoiceSettings";
import { NoiseGateSettings } from "./NoiseGateSettings";
import { EqSettings } from "./EqSettings";
import { AutoLevelSettings } from "./AutoLevelSettings";

export function SettingsPanel() {
  const locked = useSettingsLocked();
  return (
    <fieldset className="settings" disabled={locked}>
      <div className="grid">
        <DeviceSettings />
        <VoiceSettings />
      </div>
      <DeEsserSettings />
      <NoiseGateSettings />
      <EqSettings />
      <AutoLevelSettings />
    </fieldset>
  );
}
