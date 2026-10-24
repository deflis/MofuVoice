import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";
import { connectTheme } from "./theme";
import { activeMainConnection, connectMainWindow } from "./windows/main/listener";
import { activeSettingsConnection, connectSettingsWindow } from "./windows/settings/listener";
import { activeLogConnection, connectLogWindow, setLogging } from "./windows/log/listener";
const windowKind = new URLSearchParams(window.location.search).get("window");
function connectCurrentWindow() {
  if (windowKind === "settings") {
    return connectSettingsWindow();
  }
  if (windowKind === "log") {
    return connectLogWindow();
  }
  return connectMainWindow();
}
function activeConnection() {
  if (windowKind === "settings") {
    return activeSettingsConnection();
  }
  if (windowKind === "log") {
    return activeLogConnection();
  }
  return activeMainConnection();
}
const disconnectTheme = connectTheme();
connectCurrentWindow();
window.addEventListener(
  "pagehide",
  () => {
    disconnectTheme();
    if (windowKind === "log") {
      void setLogging(false);
    }
    void activeConnection()
      ?.close()
      .catch((error: unknown) => console.error(error));
  },
  { once: true },
);
const root = document.getElementById("root");
if (!root) {
  throw new Error("画面の描画先 #root が見つかりません。");
}
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
