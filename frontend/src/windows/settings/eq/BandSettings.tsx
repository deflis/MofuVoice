import styles from "../settings.module.css";
import eqStyles from "./EqSettings.module.css";
import { useId } from "react";
import { NumberField } from "../NumberField";
import type { EqBand } from "../../../types";

type SimpleProps = {
  title: string;
  left: string;
  right: string;
  value: number;
  onChange: (value: number) => void;
};
export function EqSimpleBand({ title, left, right, value, onChange }: SimpleProps) {
  const id = useId();
  const valid = Number.isFinite(value) && Math.abs(value) <= 12;
  return (
    <div className={eqStyles["eq-simple-band"]}>
      <label htmlFor={id}>{title} </label>
      <input
        id={id}
        type="range"
        min="-12"
        max="12"
        step="0.5"
        value={valid ? value : 0}
        disabled={!valid}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
      />
      <div className={styles["eq-scale"]}>
        <span>{left}</span>
        <span>そのまま</span>
        <span>{right}</span>
      </div>
    </div>
  );
}

type DetailsProps = {
  title: string;
  band: EqBand;
  setFrequency: (value: number) => void;
  setGainDb: (value: number) => void;
  setQ: (value: number) => void;
};
export function EqBandDetails({ title, band, setFrequency, setGainDb, setQ }: DetailsProps) {
  return (
    <fieldset className={eqStyles["eq-pro-band"]}>
      <legend>{title}</legend>
      <div className={eqStyles["eq-fields"]}>
        <NumberField
          label="周波数（Hz）"
          value={band.frequency}
          onChange={setFrequency}
          min={40}
          max={16000}
          step={1}
        />
        <NumberField
          label="ゲイン（dB）"
          value={band.gainDb}
          onChange={setGainDb}
          min={-12}
          max={12}
          step={0.1}
        />
        <NumberField
          label="帯域幅（Q値）"
          value={band.q}
          onChange={setQ}
          min={0.2}
          max={8}
          step={0.1}
        />
      </div>
    </fieldset>
  );
}
