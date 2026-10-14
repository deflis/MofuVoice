import { LogApp } from "./windows/log/App";
import { MainApp } from "./windows/main/App";
import { SettingsApp } from "./windows/settings/App";

const windowKind = new URLSearchParams(window.location.search).get("window");

export function App() {
  if (windowKind === "settings") {
    return <SettingsApp />;
  }
  if (windowKind === "log") {
    return <LogApp />;
  }
  return <MainApp />;
}
