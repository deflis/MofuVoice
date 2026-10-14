import { useEffect } from "react";
import { connectLog } from "./connect";
import { Notifications } from "../../components/Notifications";
import { StatusDetails } from "../../components/StatusDetails";

export function LogApp() {
  useEffect(() => connectLog().disconnect, []);

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
