import { useId } from "react";

type SemitonesInputProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
} & ({ precise: true } | { precise: false; lowerLabel: string; upperLabel: string });

export function SemitonesInput(props: SemitonesInputProps) {
  const { label, value, onChange, precise } = props;
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
          <span>{props.lowerLabel}</span>
          <span>元の声</span>
          <span>{props.upperLabel}</span>
        </div>
      )}
    </div>
  );
}
