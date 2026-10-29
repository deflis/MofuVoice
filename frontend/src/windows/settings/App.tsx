import { useTranslation } from "react-i18next";
import { useSettingsConnection, connectSettingsWindow } from "./listener";
import { Notifications } from "./Notifications";
import { SettingsWindow } from "./SettingsWindow";

export function SettingsApp() {
  const { t } = useTranslation();
  const status = useSettingsConnection((state) => state.status);

  let body;
  if (status === "loading") {
    body = (
      <div>
        <p>{t("設定を読み込んでいます…")}</p>
      </div>
    );
  } else if (status === "loadFailed") {
    body = (
      <div>
        <button type="button" onClick={() => connectSettingsWindow()}>
          {t("設定の読み込みを再試行")}
        </button>
      </div>
    );
  } else {
    body = <SettingsWindow />;
  }

  return (
    <main>
      <Notifications />
      {body}
    </main>
  );
}
