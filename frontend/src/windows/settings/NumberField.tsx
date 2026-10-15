import { NumberInput } from "./NumberInput";

type Props = {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number | "any";
  onChange: (value: number) => void;
  help?: string;
};

export function NumberField({ label, value, min, max, step, onChange, help }: Props) {
  return (
    <label>
      {label}
      <NumberInput
        required
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
      />
      {help && <small>{help}</small>}
    </label>
  );
}
