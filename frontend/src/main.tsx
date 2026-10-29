import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import { i18n } from "./i18n";
import { LanguageEffects } from "./i18n/LanguageEffects";
import { App } from "./App";
import "./styles.css";
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
connectCurrentWindow();
window.addEventListener(
  "pagehide",
  () => {
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
    <I18nextProvider i18n={i18n}>
      <LanguageEffects />
      <App />
    </I18nextProvider>
  </StrictMode>,
);
