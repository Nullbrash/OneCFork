import { useState } from "react";
import { t } from "../i18n";
import { hasExact, suggest } from "../lib/match";

interface Props<T> {
  items: readonly T[];
  name: (item: T) => string;
  /** Дополнительная строка в подсказке (например, компания человека). */
  hint?: (item: T) => string;
  exclude?: (item: T) => boolean;
  placeholder: string;
  onPick: (item: T) => void;
  /** Если задано — в конце подсказок «Создать „…“». */
  onCreate?: (name: string) => void;
}

/** Ввод с подсказками из уже существующих значений и созданием нового. */
export function Suggest<T>({ items, name, hint, exclude, placeholder, onPick, onCreate }: Props<T>) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const matches = suggest(items, query, name, exclude);
  const canCreate = !!onCreate && query.trim() !== "" && !hasExact(items, query, name);
  const total = matches.length + (canCreate ? 1 : 0);

  const choose = (index: number) => {
    if (index < matches.length) onPick(matches[index]);
    else if (canCreate) onCreate?.(query.trim());
    setQuery("");
    setActive(0);
  };

  return (
    <div className="suggest">
      <input
        value={query}
        placeholder={placeholder}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") setActive((a) => Math.min(a + 1, Math.max(total - 1, 0)));
          else if (e.key === "ArrowUp") setActive((a) => Math.max(a - 1, 0));
          else if (e.key === "Enter" && total > 0) choose(active);
          else if (e.key === "Escape") setQuery("");
          else return;
          e.preventDefault();
          e.stopPropagation();
        }}
      />
      {total > 0 && (
        <ul className="suggest-list">
          {matches.map((item, i) => (
            <li key={i} className={i === active ? "active" : ""} onMouseDown={() => choose(i)}>
              {name(item)}
              {hint && hint(item) && <span className="muted"> — {hint(item)}</span>}
            </li>
          ))}
          {canCreate && (
            <li
              className={active === matches.length ? "active create" : "create"}
              onMouseDown={() => choose(matches.length)}
            >
              {t.card.create(query.trim())}
            </li>
          )}
        </ul>
      )}
    </div>
  );
}
