import { useEffect } from "react";
import { useLogStore } from "./store";
import { Notifications } from "./Notifications";
import { StatusDetails } from "./StatusDetails";

export function LogApp() {
  useEffect(() => useLogStore.getState().mount(), []);

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
