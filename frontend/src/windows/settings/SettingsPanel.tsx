import { connectSettingsNavigation, readSettingsSection, type SettingsSection } from "./navigation";
import { useEffect, useState } from "react";
import { useSettingsLocked } from "./hooks";
import { DeviceSettings } from "./DeviceSettings";
import { DeEsserSettings } from "./DeEsserSettings";
import { VoiceSettings } from "./VoiceSettings";
import { NoiseGateSettings } from "./NoiseGateSettings";
import { EqSettings } from "./EqSettings";
import { AutoLevelSettings } from "./AutoLevelSettings";
import { RumbleSettings } from "./RumbleSettings";

const groups: { id: SettingsSection; name: string }[] = [
  { id: "devices", name: "音声デバイス" },
  { id: "voice", name: "声を変える" },
  { id: "noise", name: "雑音を減らす" },
  { id: "tone", name: "声を整える" },
];

export function SettingsPanel() {
  const locked = useSettingsLocked();
  const [group, setGroup] = useState<SettingsSection>(readSettingsSection);
  useEffect(() => connectSettingsNavigation(setGroup), []);
  return (
    <div className="settings-layout">
      <nav aria-label="設定カテゴリ">
        {groups.map((item) => (
          <button
            key={item.name}
            type="button"
            aria-current={group === item.id ? "page" : undefined}
            onClick={() => setGroup(item.id)}
          >
            {item.name}
          </button>
        ))}
      </nav>
      <div className="settings-content">
        <fieldset className="settings" disabled={locked}>
          {group === "devices" && <DeviceSettings />}
          {group === "voice" && <VoiceSettings />}
          {group === "noise" && (
            <>
              <RumbleSettings />
              <NoiseGateSettings />
              <DeEsserSettings />
            </>
          )}
          {group === "tone" && (
            <>
              <EqSettings />
              <AutoLevelSettings />
            </>
          )}
        </fieldset>
      </div>
    </div>
  );
}
