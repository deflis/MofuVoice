import { useTranslation } from "react-i18next";
import { useSaveSettings } from "./hooks";

export function SaveSettingsButton() {
  const { t } = useTranslation();
  const { disabled, save } = useSaveSettings();
  return (
    <footer>
      <button type="button" disabled={disabled} onClick={() => void save()}>
        {t("設定を保存")}
      </button>
    </footer>
  );
}
