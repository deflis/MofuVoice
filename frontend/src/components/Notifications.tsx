import { useNotifications } from "../audioHooks";

export function Notifications() {
  const { error, notice } = useNotifications();
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
