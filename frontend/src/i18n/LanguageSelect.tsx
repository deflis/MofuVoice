import { setLocale } from "./index";
import { useTranslation } from "react-i18next";

export function LanguageSelect() {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage;
  return (
    <label className="language-select">
      <span>{t("表示言語")}</span>
      <select
        value={locale}
        onChange={(event) => {
          const value = event.target.value;
          if (value === "ja" || value === "en") {
            setLocale(value);
          }
        }}
      >
        <option value="ja" lang="ja">
          日本語
        </option>
        <option value="en" lang="en">
          English
        </option>
      </select>
    </label>
  );
}
