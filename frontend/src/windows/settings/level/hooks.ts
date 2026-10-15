import { useSettingsStore } from "../store";
import { useSettingsConnection } from "../listener";
export function useAutoLevelSettings() {
  const level = useSettingsStore((state) => state.level);
  const levelDefaults = useSettingsConnection((state) => state.levelDefaults);
  function resetLevel() {
    if (!levelDefaults) {
      return;
    }
    level.setTargetDb(levelDefaults.targetDb);
    level.setMaxGainDb(levelDefaults.maxGainDb);
    level.setFloorDb(levelDefaults.floorDb);
    level.setResponseMs(levelDefaults.responseMs);
    level.setThresholdDb(levelDefaults.thresholdDb);
    level.setRatio(levelDefaults.ratio);
    level.setAttackMs(levelDefaults.attackMs);
    level.setReleaseMs(levelDefaults.releaseMs);
  }
  return { level, levelDefaults, resetLevel };
}
