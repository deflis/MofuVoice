import { useTranslation } from "react-i18next";
import styles from "./AudioControls.module.css";
import { useMainStore } from "./store";
import { startAudio, stopAudio } from "./listener";
import { canStart, canStop, togglePending } from "./selectors";

export function AudioControls() {
  const { t } = useTranslation();
  const busy = useMainStore((state) => state.status === "running" || state.status === "stopping");
  const pending = useMainStore(togglePending);
  const startAllowed = useMainStore(canStart);
  const stopAllowed = useMainStore(canStop);
  const actionLabel = busy ? t("音声を停止") : t("音声を開始");
  return (
    <section>
      <div className={styles.actions}>
        <button
          type="button"
          className={styles["audio-toggle"]}
          disabled={busy ? !stopAllowed : !startAllowed}
          onClick={() => void (busy ? stopAudio() : startAudio(false))}
        >
          {pending ? t("切り替え中…") : actionLabel}
        </button>
      </div>
    </section>
  );
}
