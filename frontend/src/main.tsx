import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";
import { connectMainWindow } from "./windows/main/listener";
import { connectSettingsWindow } from "./windows/settings/listener";
import { connectLogWindow, setLogging } from "./windows/log/listener";
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
const connection = connectCurrentWindow();
window.addEventListener(
  "pagehide",
  () => {
    if (windowKind === "log") {
      void setLogging(false);
    }
    void connection.close().catch((error: unknown) => console.error(error));
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
