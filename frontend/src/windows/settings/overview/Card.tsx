import styles from "./overview.module.css";
import type { ReactNode } from "react";

export type CardProps = { onSelect: (id: string) => void };
export function Card({
  id,
  title,
  enabled,
  onSelect,
  children,
}: CardProps & {
  id: string;
  title: string;
  enabled: boolean;
  children: ReactNode;
}) {
  return (
    <article
      className={styles["filter-card"]}
      data-enabled={enabled}
      aria-labelledby={`filter-${id}`}
    >
      <div className={styles["filter-card-heading"]}>
        <h3 id={`filter-${id}`}>{title}</h3>
        <span className={styles["filter-state"]}>{enabled ? "有効" : "無効 · バイパス中"}</span>
      </div>
      {children}
      <button type="button" onClick={() => onSelect(id)} aria-label={`${title}の個別設定を開く`}>
        個別設定を開く →
      </button>
    </article>
  );
}
export function InvalidGraph() {
  return (
    <output className={styles["filter-invalid"]}>
      設定値が不正または入力途中のため、図を表示できません。個別設定で確認してください。
    </output>
  );
}
export function Value({ children }: { children: ReactNode }) {
  return <p className={styles["filter-values"]}>{children}</p>;
}
export function num(value: number) {
  return Number.isFinite(value) ? Number(value.toFixed(2)).toLocaleString("ja-JP") : "未入力";
}
