import { useState, type InputHTMLAttributes } from "react";

type Props = Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "type"> & { value: number };
export function NumberInput({ value, onChange, onBlur, ...props }: Props) {
  const [draft, setDraft] = useState<{ number: number; text: string }>();
  let text = Number.isNaN(value) ? "" : String(value);
  if (draft && Object.is(draft.number, value)) {
    text = draft.text;
  }
  return (
    <input
      {...props}
      type="number"
      value={text}
      onChange={(event) => {
        setDraft({ number: event.currentTarget.valueAsNumber, text: event.currentTarget.value });
        onChange?.(event);
      }}
      onBlur={(event) => {
        setDraft(undefined);
        onBlur?.(event);
      }}
    />
  );
}
