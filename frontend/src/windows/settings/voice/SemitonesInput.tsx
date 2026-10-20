import styles from "../../../styles.module.css";
import { NumberInput } from "../NumberInput";
import { useId } from "react";

export function SemitonesInput({
  label,
  value,
  onChange,
  lowerLabel,
  upperLabel,
}: {
  lowerLabel: string;
  upperLabel: string;
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const number = value;
  const sliderId = useId();
  return (
    <div className={styles.semitones}>
      <label htmlFor={sliderId}>
        {label}
        <small>半音</small>
      </label>
      <div className={styles["slider-row"]}>
        <input
          id={sliderId}
          type="range"
          min="-12"
          max="12"
          step="0.01"
          value={Number.isFinite(number) ? number : 0}
          onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
        />
        <NumberInput
          aria-label={`${label}の値`}

          min="-12"
          max="12"
          step="0.01"
          required
          value={value}
          onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
        />
        <div className={styles["eq-scale"]}>
          <span>{lowerLabel}</span>
          <span>元の声</span>
          <span>{upperLabel}</span>
        </div>
      </div>
    </div>
  );
}
