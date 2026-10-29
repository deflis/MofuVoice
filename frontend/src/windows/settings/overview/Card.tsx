import { useTranslation } from "react-i18next";
import styles from "./overview.module.css";
import type { ReactNode } from "react";

export type CardProps = { onSelect: (id: string) => void; locked: boolean };
export type FilterToggle = {
  label: string;
  enabled: boolean;
  onChange: (enabled: boolean) => void;
};
export function Card({
  id,
  title,
  toggles,
  locked,
  onSelect,
  children,
}: CardProps & {
  id: string;
  title: string;
  toggles: FilterToggle[];
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const enabled = toggles.some((toggle) => toggle.enabled);
  return (
    <article
      className={styles["filter-card"]}
      data-enabled={enabled}
      aria-labelledby={`filter-${id}`}
    >
      <div className={styles["filter-card-heading"]}>
        <h3 id={`filter-${id}`}>{t(title)}</h3>
        <div className={styles["filter-card-controls"]}>
          {toggles.map((toggle) => (
            <label key={toggle.label}>
              <input
                type="checkbox"
                checked={toggle.enabled}
                disabled={locked}
                aria-label={`${title}: ${toggle.label}`}
                onChange={(event) => toggle.onChange(event.target.checked)}
              />
              {toggle.label}
            </label>
          ))}
        </div>
      </div>
      {enabled && children}
      <button
        type="button"
        onClick={() => onSelect(id)}
        aria-label={t("{0}の個別設定を開く", { v0: t(title) })}
      >
        {t("個別設定を開く →")}
      </button>
    </article>
  );
}
export function InvalidGraph() {
  const { t } = useTranslation();
  return (
    <output className={styles["filter-invalid"]}>
      {t("設定値が不正または入力途中のため、図を表示できません。個別設定で確認してください。")}
    </output>
  );
}
export function Value({ children }: { children: ReactNode }) {
  return <p className={styles["filter-values"]}>{children}</p>;
}
export function num(value: number) {
  return Number.isFinite(value) ? Number(value.toFixed(2)).toLocaleString() : "—";
}
