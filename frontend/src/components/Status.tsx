import { useHeaderStatus, useNotifications, useStatusDetails } from "../settingsHooks";

export function AppHeader() {
  const status = useHeaderStatus();
  return (
    <header>
      <h1>Voice Changer</h1>
      <span>{status}</span>
    </header>
  );
}

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

export function StatusDetails() {
  const status = useStatusDetails();
  return (
    <details>
      <summary>処理状況・レイテンシの詳細</summary>
      <pre>{status}</pre>
    </details>
  );
}
