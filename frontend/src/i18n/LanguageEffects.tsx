import { useEffect } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import { connectLanguageStorage } from "./index";

export function LanguageEffects() {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? "en";
  useEffect(connectLanguageStorage, []);
  useEffect(() => {
    document.documentElement.lang = locale;
    const kind = new URLSearchParams(window.location.search).get("window");
    const name = t("もふボイス");
    let title = name;
    if (kind === "settings") {
      title = `${t("設定")} — ${name}`;
    } else if (kind === "log") {
      title = `${t("ログ")} — ${name}`;
    }
    document.title = title;
    if (isTauri()) {
      void getCurrentWindow().setTitle(title).catch(console.error);
    }
  }, [locale, t]);
  return null;
}
