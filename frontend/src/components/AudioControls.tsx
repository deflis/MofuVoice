import { useAudioControls } from "../audioHooks";
import { useAudioStore } from "../audioStore";

export function AudioControls() {
  const { startDisabled, stopDisabled, start, stop } = useAudioControls();
  const busy = useAudioStore((state) => state.snapshot.busy);
  const pending = useAudioStore((state) => state.pending);
  const actionLabel = busy ? "音声を停止" : "音声を開始";
  return (
    <section>
      <div className="actions">
        <button
          type="button"
          className="audio-toggle"
          disabled={busy ? stopDisabled : startDisabled}
          onClick={() => void (busy ? stop() : start(false))}
        >
          {pending ? "切り替え中…" : actionLabel}
        </button>
      </div>
    </section>
  );
}
