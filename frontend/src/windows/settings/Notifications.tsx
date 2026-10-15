import { useSettingsStore } from "./store";

export function Notifications() {
  const error = useSettingsStore((state) => state.error || state.deviceError);
  const notice = useSettingsStore((state) => state.notice);
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
