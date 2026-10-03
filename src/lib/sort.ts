import type { ListRow } from "../api";
import { cellText, type ColumnId } from "./columns";

export interface SortState {
  column: ColumnId;
  dir: "asc" | "desc";
}

export const DEFAULT_SORT: SortState = { column: "title", dir: "asc" };

const collator = new Intl.Collator("ru", { sensitivity: "base", numeric: true });

/** Пустые значения — всегда в конце, в любом направлении: иначе при
 * сортировке «по убыванию» список начинался бы с пустых строк. */
export function sortRows(rows: readonly ListRow[], sort: SortState): ListRow[] {
  const sign = sort.dir === "asc" ? 1 : -1;
  return [...rows].sort((a, b) => {
    const x = cellText(a, sort.column);
    const y = cellText(b, sort.column);
    if (!x && y) return 1;
    if (x && !y) return -1;
    const byColumn = collator.compare(x, y) * sign;
    return byColumn !== 0 ? byColumn : collator.compare(a.title, b.title);
  });
}

export function nextSort(current: SortState, column: ColumnId): SortState {
  if (current.column !== column) return { column, dir: "asc" };
  return { column, dir: current.dir === "asc" ? "desc" : "asc" };
}
