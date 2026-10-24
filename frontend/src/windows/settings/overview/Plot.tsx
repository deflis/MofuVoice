import styles from "./overview.module.css";
import { useId, type ReactNode } from "react";

export function Plot({
  description,
  children,
  xLabel,
  yLabel,
}: {
  description: string;
  children: ReactNode;
  xLabel: string;
  yLabel: string;
}) {
  const id = useId();
  return (
    // SVG is an inline graphic, not an external img resource.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <svg className={styles["filter-plot"]} viewBox="0 0 320 112" role="img" aria-labelledby={id}>
      <title id={id}>{description}</title>
      <path className={styles["filter-axis"]} d="M36 12V86H308" />
      <text x="36" y="10">
        {yLabel}
      </text>
      <text x="308" y="107" textAnchor="end">
        {xLabel}
      </text>
      {children}
    </svg>
  );
}
export function curvePath(points: [number, number][]) {
  return points.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(3)},${y.toFixed(3)}`).join(" ");
}
const x = (value: number) => 36 + ((value + 60) / 60) * 272;
const y = (value: number) => 86 - ((value + 60) / 60) * 64;
export function TransferPlot({
  threshold,
  ratio,
  description,
}: {
  threshold: number;
  ratio: number;
  description: string;
}) {
  const output = ratio === Infinity ? threshold : threshold + (0 - threshold) / ratio;
  return (
    <Plot description={description} xLabel="入力 −60 → 0 dBFS" yLabel="出力 dBFS">
      <path className={styles["filter-reference"]} d="M36 86L308 22" />
      <path
        className={styles["filter-curve"]}
        d={curvePath([
          [36, 86],
          [x(threshold), y(threshold)],
          [308, y(output)],
        ])}
      />
      <circle className={styles["filter-point"]} cx={x(threshold)} cy={y(threshold)} r="3" />
    </Plot>
  );
}
