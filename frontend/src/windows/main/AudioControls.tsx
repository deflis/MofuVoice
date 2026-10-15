import { useMainStore } from "./store";
import { canStart, canStop, togglePending } from "./selectors";

export function AudioControls() {
  const machine = useMainStore((state) => state.machine);
  const start = useMainStore((state) => state.start);
  const stop = useMainStore((state) => state.stop);
  const busy = machine.status === "running" || machine.status === "stopping";
  const pending = togglePending(machine);
  const actionLabel = busy ? "音声を停止" : "音声を開始";
  return (
    <section>
      <div className="actions">
        <button
          type="button"
          className="audio-toggle"
          disabled={busy ? !canStop(machine) : !canStart(machine)}
          onClick={() => void (busy ? stop() : start(false))}
        >
          {pending ? "切り替え中…" : actionLabel}
        </button>
      </div>
    </section>
  );
}
