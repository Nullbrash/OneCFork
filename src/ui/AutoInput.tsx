import { useEffect, useRef, useState, type InputHTMLAttributes } from "react";

const SAVE_DELAY_MS = 700;

type Props = {
  value: string;
  /** Вызывается после паузы в наборе и при уходе из поля, если значение изменилось. */
  onCommit: (value: string) => void | Promise<void>;
  multiline?: boolean;
  /** Пустое значение не сохраняется — поле возвращается к прежнему. */
  required?: boolean;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange">;

/** Поле с автосохранением: кнопки «Сохранить» нет. */
export function AutoInput({ value, onCommit, multiline, required, ...rest }: Props) {
  const [draft, setDraft] = useState(value);
  const saved = useRef(value);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    // Значение пришло снаружи (перезагрузка карточки) — принять, если человек
    // сейчас не держит в поле несохранённую правку.
    if (draft === saved.current) setDraft(value);
    saved.current = value;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  const commit = (next: string) => {
    window.clearTimeout(timer.current);
    if (next === saved.current) return;
    if (required && !next.trim()) {
      setDraft(saved.current);
      return;
    }
    saved.current = next;
    void onCommit(next);
  };

  const change = (next: string) => {
    setDraft(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => commit(next), SAVE_DELAY_MS);
  };

  useEffect(() => () => window.clearTimeout(timer.current), []);

  if (multiline) {
    return (
      <textarea
        className={rest.className}
        placeholder={rest.placeholder}
        value={draft}
        rows={4}
        onChange={(e) => change(e.target.value)}
        onBlur={() => commit(draft)}
      />
    );
  }
  return (
    <input
      {...rest}
      value={draft}
      onChange={(e) => change(e.target.value)}
      onBlur={() => commit(draft)}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit(draft);
        rest.onKeyDown?.(e);
      }}
    />
  );
}
