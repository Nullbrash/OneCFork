import type { ListRow } from "../api";

export type ColumnId = "title" | "roles" | "specializations" | "companies" | "phone";

export const ALL_COLUMNS: readonly ColumnId[] = ["title", "roles", "specializations", "companies", "phone"];

export interface ColumnSettings {
  order: ColumnId[];
  hidden: ColumnId[];
}

export const DEFAULT_COLUMNS: ColumnSettings = { order: [...ALL_COLUMNS], hidden: [] };

/** «Название» нельзя скрыть: без него строку не узнать и не открыть. */
export const LOCKED_COLUMN: ColumnId = "title";

/**
 * Настройки из базы → рабочие: неизвестные столбцы отбрасываются, новые
 * (появившиеся в следующих версиях) дописываются в конец — сохранённый
 * выбор старой версии не ломает список.
 */
export function normalizeColumns(raw: unknown): ColumnSettings {
  const isColumn = (x: unknown): x is ColumnId => ALL_COLUMNS.includes(x as ColumnId);
  const obj = (typeof raw === "object" && raw !== null ? raw : {}) as Partial<Record<string, unknown>>;
  const order = Array.isArray(obj.order) ? obj.order.filter(isColumn) : [];
  const unique = [...new Set(order)];
  for (const c of ALL_COLUMNS) if (!unique.includes(c)) unique.push(c);
  const hidden = Array.isArray(obj.hidden)
    ? [...new Set(obj.hidden.filter(isColumn))].filter((c) => c !== LOCKED_COLUMN)
    : [];
  return { order: unique, hidden };
}

export function parseColumns(json: string | null): ColumnSettings {
  if (!json) return DEFAULT_COLUMNS;
  try {
    return normalizeColumns(JSON.parse(json));
  } catch {
    return DEFAULT_COLUMNS;
  }
}

export function visibleColumns(settings: ColumnSettings, forCompanies: boolean): ColumnId[] {
  // У компании нет «компании человека» — столбец в разделе компаний пуст.
  return settings.order.filter((c) => !settings.hidden.includes(c) && !(forCompanies && c === "companies"));
}

export function toggleColumn(settings: ColumnSettings, id: ColumnId): ColumnSettings {
  if (id === LOCKED_COLUMN) return settings;
  const hidden = settings.hidden.includes(id)
    ? settings.hidden.filter((c) => c !== id)
    : [...settings.hidden, id];
  return { ...settings, hidden };
}

export function moveColumn(settings: ColumnSettings, id: ColumnId, delta: -1 | 1): ColumnSettings {
  const order = [...settings.order];
  const i = order.indexOf(id);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= order.length) return settings;
  [order[i], order[j]] = [order[j], order[i]];
  return { ...settings, order };
}

export function cellText(row: ListRow, column: ColumnId): string {
  switch (column) {
    case "title":
      return row.title;
    case "roles":
      return row.roles.join(", ");
    case "specializations":
      return row.specializations.join(", ");
    case "companies":
      return row.companies.join(", ");
    case "phone":
      return row.mainPhone ?? "";
  }
}
