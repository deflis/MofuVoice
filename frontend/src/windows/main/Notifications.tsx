import { useMainStore } from "./store";
import { machineError } from "./selectors";

export function Notifications() {
  const error = useMainStore((state) => machineError(state.machine) || state.error);
  const notice = useMainStore((state) => state.notice);
  return (
    <>
      {error && (
        <div className="error" role="alert">
          {error}
        </div>
      )}
      {notice && <output className="notice">{notice}</output>}
    </>
  );
}
