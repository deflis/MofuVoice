import { Notifications } from "./Notifications";
import { StatusDetails } from "./StatusDetails";

export function LogApp() {
  return (
    <main>
      <Notifications />
      <div className="log-window">
        <header>
          <h1>ログ</h1>
          <span className="muted">Voice Changer</span>
        </header>
        <StatusDetails />
      </div>
    </main>
  );
}
