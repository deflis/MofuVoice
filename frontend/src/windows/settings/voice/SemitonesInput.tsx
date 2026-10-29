import { useTranslation } from "react-i18next";
import styles from "../settings.module.css";
import { NumberInput } from "../NumberInput";
import { useId } from "react";
import { percentToSemitones, semitonesToPercent } from "./pitchUnits";

export function SemitonesInput({
  label,
  value,
  onChange,
  lowerLabel,
  upperLabel,
  percent = false,
}: {
  percent?: boolean;
  lowerLabel: string;
  upperLabel: string;
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const { t } = useTranslation();
  const number = percent ? Number(semitonesToPercent(value).toFixed(4)) : value;
  const change = (nextValue: number) =>
    onChange(percent ? percentToSemitones(nextValue) : nextValue);
  const neutral = percent ? 100 : 0;
  const sliderId = useId();
  return (
    <div className={styles.semitones}>
      <label htmlFor={sliderId}>
        {label}
        <small>{percent ? "%" : t("半音")}</small>
      </label>
      <div className={styles["slider-row"]}>
        <input
          id={sliderId}
          type="range"
          min={percent ? 50 : -12}
          max={percent ? 200 : 12}
          step={percent ? 0.1 : 0.01}
          value={Number.isFinite(number) ? number : neutral}
          onChange={(event) => change(event.currentTarget.valueAsNumber)}
        />
        <NumberInput
          aria-label={t("{0}の値（{1}）", { v0: label, v1: percent ? "%" : t("半音") })}
          min={percent ? 50 : -12}
          max={percent ? 200 : 12}
          step={percent ? "any" : 0.01}
          required
          value={number}
          onChange={(event) => change(event.currentTarget.valueAsNumber)}
        />
        <div className={styles["eq-scale"]}>
          <span>{lowerLabel}</span>
          <span>
            {t("元の声")}
            {percent ? "（100%）" : ""}
          </span>
          <span>{upperLabel}</span>
        </div>
      </div>
    </div>
  );
}
