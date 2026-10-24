import styles from "./settings.module.css";
import { CompressorSettings } from "./compressor/CompressorSettings";
import { FilterOverview } from "./overview/FilterOverview";
import { LimiterSettings } from "./limiter/LimiterSettings";
import { DereverbSettings } from "./dereverb/DereverbSettings";
import { ReverbSettings } from "./reverb/ReverbSettings";
import {
  connectSettingsNavigation,
  readSettingsSection,
  settingsSections,
  type SettingsSection,
} from "./navigation";
import { useEffect, useState } from "react";
import { useSettingsLocked } from "./hooks";
import { DeviceSettings } from "./devices/DeviceSettings";
import { DeEsserSettings } from "./deesser/DeEsserSettings";
import { VoiceSettings } from "./voice/VoiceSettings";
import { NoiseGateSettings } from "./noise/NoiseGateSettings";
import { EqSettings } from "./eq/EqSettings";
import { AutoLevelSettings } from "./level/AutoLevelSettings";
import { RumbleSettings } from "./noise/RumbleSettings";

const groups: { id: SettingsSection; name: string }[] = [
  { id: "overview", name: "フィルタ構成" },
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
    <div className={styles["settings-layout"]}>
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
      <div className={styles["settings-content"]}>
        {group === "overview" ? (
          <FilterOverview
            onSelect={(stageId) => {
              const section = settingsSections.find((candidate) => candidate === stageId);
              if (section) {
                setGroup(section);
              }
            }}
          />
        ) : (
          <>
            <button type="button" onClick={() => setGroup("overview")}>
              フィルタ構成へ戻る
            </button>
            <fieldset className={styles.settings} disabled={locked}>
              {group === "inputEq" && (
                <>
                  <EqSettings filter="inputEq" title="入力EQ" />
                  <RumbleSettings />
                </>
              )}
              {group === "preConversionEq" && (
                <EqSettings filter="preConversionEq" title="変換前EQ" />
              )}
              {group === "postConversionEq" && <EqSettings />}
              {group === "noiseGate" && <NoiseGateSettings />}
              {group === "deesser" && <DeEsserSettings />}
              {group === "level" && <AutoLevelSettings />}
              {group === "compressor" && <CompressorSettings />}
              {group === "postCompressor" && <AutoLevelSettings part="compressor" />}
              {group === "autoLevel" && <AutoLevelSettings part="auto" />}
              {group === "limiter" && <LimiterSettings />}
              {group === "dereverb" && <DereverbSettings />}
              {group === "reverb" && <ReverbSettings />}
              {group === "devices" && <DeviceSettings />}
              {group === "voice" && <VoiceSettings />}
              {group === "noise" && (
                <>
                  <NoiseGateSettings />
                  <DeEsserSettings />
                  <DereverbSettings />
                </>
              )}
              {group === "tone" && (
                <>
                  <EqSettings filter="inputEq" title="入力EQ" />
                  <RumbleSettings />
                  <LimiterSettings />
                  <CompressorSettings />
                  <EqSettings filter="preConversionEq" title="変換前EQ" />
                  <EqSettings />
                  <ReverbSettings />
                  <AutoLevelSettings />
                </>
              )}
            </fieldset>
          </>
        )}
      </div>
    </div>
  );
}
