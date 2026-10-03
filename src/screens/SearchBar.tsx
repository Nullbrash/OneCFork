import { useEffect, useRef, useState } from "react";
import { api, type Term } from "../api";
import { t } from "../i18n";
import { NO_FILTERS, type SearchFilters } from "../lib/search";
import { useToasts } from "../ui/toasts";

type FilterKey = keyof SearchFilters;

const FILTERS: {
  key: FilterKey;
  vocabulary: "roles" | "specializations" | "addressLabels";
  label: string;
}[] = [
  { key: "roleIds", vocabulary: "roles", label: t.search.roles },
  { key: "specializationIds", vocabulary: "specializations", label: t.search.specializations },
  { key: "addressLabelIds", vocabulary: "addressLabels", label: t.search.addressLabels },
];

interface Props {
  query: string;
  onQuery: (q: string) => void;
  filters: SearchFilters;
  onFilters: (f: SearchFilters) => void;
  /** Не 0 — поле поиска получает фокус (Ctrl + F); затем сигнал гасится. */
  focusToken: number;
  onFocused: () => void;
  /** Меняется после правок — словари фильтров перечитываются. */
  revision: number;
}

export function SearchBar({ query, onQuery, filters, onFilters, focusToken, onFocused, revision }: Props) {
  const toasts = useToasts();
  const input = useRef<HTMLInputElement>(null);
  const [terms, setTerms] = useState<Record<FilterKey, Term[]>>({
    roleIds: [],
    specializationIds: [],
    addressLabelIds: [],
  });
  const [open, setOpen] = useState<FilterKey | null>(null);

  useEffect(() => {
    Promise.all(FILTERS.map((f) => api.listTerms(f.vocabulary)))
      .then(([roleIds, specializationIds, addressLabelIds]) =>
        setTerms({ roleIds, specializationIds, addressLabelIds }),
      )
      .catch(toasts.error);
  }, [revision, toasts.error]);

  useEffect(() => {
    if (focusToken === 0) return;
    input.current?.focus();
    input.current?.select();
    onFocused();
  }, [focusToken, onFocused]);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [open]);

  const toggle = (key: FilterKey, id: number) => {
    const chosen = filters[key];
    onFilters({ ...filters, [key]: chosen.includes(id) ? chosen.filter((x) => x !== id) : [...chosen, id] });
  };

  const chips = FILTERS.flatMap((f) =>
    filters[f.key].map((id) => ({
      key: f.key,
      id,
      name: terms[f.key].find((x) => x.id === id)?.name ?? "…",
    })),
  );

  return (
    <div className="search-bar">
      <div className="search-row">
        <div className="search-input">
          <input
            ref={input}
            value={query}
            placeholder={t.search.placeholder}
            onChange={(e) => onQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== "Escape") return;
              // Esc: сначала очистить запрос, повторный — выйти из поля.
              e.stopPropagation();
              if (query) onQuery("");
              else input.current?.blur();
            }}
          />
          {query && (
            <button type="button" className="clear" title={t.search.clear} onClick={() => onQuery("")}>
              ×
            </button>
          )}
        </div>
        <div className="filters" onClick={(e) => e.stopPropagation()}>
          {FILTERS.map((f) => (
            <div key={f.key} className="menu-anchor">
              <button
                type="button"
                className={filters[f.key].length ? "active-filter" : undefined}
                onClick={() => setOpen(open === f.key ? null : f.key)}
              >
                {f.label}
                {filters[f.key].length > 0 && ` (${filters[f.key].length})`} ▾
              </button>
              {open === f.key && (
                <div className="menu filter-menu">
                  {terms[f.key].length === 0 && <div className="menu-title">{t.search.noTerms}</div>}
                  {terms[f.key].map((term) => (
                    <label key={term.id} className="menu-option">
                      <span>
                        <input
                          type="checkbox"
                          checked={filters[f.key].includes(term.id)}
                          onChange={() => toggle(f.key, term.id)}
                        />{" "}
                        {term.name}
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
      {chips.length > 0 && (
        <div className="chips filter-chips">
          {chips.map((c) => (
            <span key={`${c.key}-${c.id}`} className="chip on">
              {c.name}
              <button
                type="button"
                className="chip-x"
                title={t.card.remove}
                onClick={() => toggle(c.key, c.id)}
              >
                ×
              </button>
            </span>
          ))}
          <button
            type="button"
            className="link"
            onClick={() => {
              onFilters(NO_FILTERS);
              onQuery("");
            }}
          >
            {t.search.resetAll}
          </button>
        </div>
      )}
    </div>
  );
}
