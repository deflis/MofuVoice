import { useId } from "react";

export function SemitonesInput({
  label,
  value,
  onChange,
  precise = false,
  lowerLabel = "低く",
  upperLabel = "高く",
}: {
  precise?: boolean;
  lowerLabel?: string;
  upperLabel?: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const number = Number(value);
  const sliderId = useId();
  return (
    <div className="semitones">
      <label htmlFor={sliderId}>
        {label}
        {precise && <small>半音</small>}
      </label>
      <div className={precise ? "slider-row" : "simple-slider-row"}>
        <input
          id={sliderId}
          type="range"
          min="-12"
          max="12"
          step="0.01"
          value={Number.isFinite(number) ? number : 0}
          onChange={(event) => onChange(event.target.value)}
        />
        {precise && (
          <input
            aria-label={`${label}の値`}
            type="number"
            min="-12"
            max="12"
            step="0.01"
            required
            value={value}
            onChange={(event) => onChange(event.target.value)}
          />
        )}
      </div>
      {!precise && (
        <div className="eq-scale">
          <span>{lowerLabel}</span>
          <span>元の声</span>
          <span>{upperLabel}</span>
        </div>
      )}
    </div>
  );
}
