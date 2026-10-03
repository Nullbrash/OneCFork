import { describe, expect, it } from "vitest";
import type { ListRow } from "../api";
import {
  DEFAULT_COLUMNS,
  moveColumn,
  normalizeColumns,
  parseColumns,
  toggleColumn,
  visibleColumns,
} from "./columns";
import { hasExact, suggest, textKey } from "./match";
import { nextSort, sortRows } from "./sort";
import { parseTheme, resolveTheme } from "./theme";

const row = (id: number, title: string, extra: Partial<ListRow> = {}): ListRow => ({
  id,
  kind: "company",
  title,
  roles: [],
  specializations: [],
  companies: [],
  mainPhone: null,
  ...extra,
});

describe("columns", () => {
  it("drops unknown columns and appends missing ones", () => {
    const s = normalizeColumns({ order: ["phone", "bogus", "title", "phone"], hidden: ["roles", "x"] });
    expect(s.order).toEqual(["phone", "title", "roles", "specializations", "companies"]);
    expect(s.hidden).toEqual(["roles"]);
  });

  it("never hides the title column", () => {
    expect(normalizeColumns({ hidden: ["title"] }).hidden).toEqual([]);
    expect(toggleColumn(DEFAULT_COLUMNS, "title")).toBe(DEFAULT_COLUMNS);
  });

  it("falls back to defaults on broken json", () => {
    expect(parseColumns("{not json")).toEqual(DEFAULT_COLUMNS);
    expect(parseColumns(null)).toEqual(DEFAULT_COLUMNS);
  });

  it("toggles, moves and hides the company column for companies", () => {
    const hidden = toggleColumn(DEFAULT_COLUMNS, "roles");
    expect(visibleColumns(hidden, false)).not.toContain("roles");
    expect(visibleColumns(toggleColumn(hidden, "roles"), false)).toContain("roles");
    expect(moveColumn(DEFAULT_COLUMNS, "phone", -1).order.at(-2)).toBe("phone");
    expect(moveColumn(DEFAULT_COLUMNS, "title", -1)).toBe(DEFAULT_COLUMNS);
    expect(visibleColumns(DEFAULT_COLUMNS, true)).not.toContain("companies");
  });
});

describe("sortRows", () => {
  const rows = [
    row(1, "яблоко", { roles: ["Поставщик"] }),
    row(2, "Арбуз"),
    row(3, "ёлка", { roles: ["Монтажник"] }),
  ];

  it("sorts Cyrillic titles case-insensitively", () => {
    expect(sortRows(rows, { column: "title", dir: "asc" }).map((r) => r.id)).toEqual([2, 3, 1]);
    expect(sortRows(rows, { column: "title", dir: "desc" }).map((r) => r.id)).toEqual([1, 3, 2]);
  });

  it("keeps empty values last in both directions", () => {
    expect(sortRows(rows, { column: "roles", dir: "asc" }).map((r) => r.id)).toEqual([3, 1, 2]);
    expect(sortRows(rows, { column: "roles", dir: "desc" }).map((r) => r.id)).toEqual([1, 3, 2]);
  });

  it("second click on a column reverses direction", () => {
    expect(nextSort({ column: "title", dir: "asc" }, "title")).toEqual({ column: "title", dir: "desc" });
    expect(nextSort({ column: "title", dir: "desc" }, "phone")).toEqual({ column: "phone", dir: "asc" });
  });
});

describe("suggest", () => {
  const names = ["Шторы", "Карнизы для штор", "Окна", "Ткани"];
  const id = (s: string) => s;

  it("prefers prefix matches, then substring matches", () => {
    expect(suggest(names, "шт", id)).toEqual(["Шторы", "Карнизы для штор"]);
  });

  it("excludes already chosen and empty queries", () => {
    expect(suggest(names, "шт", id, (s) => s === "Шторы")).toEqual(["Карнизы для штор"]);
    expect(suggest(names, "  ", id)).toEqual([]);
  });

  it("detects exact matches ignoring case and spaces", () => {
    expect(hasExact(names, "  шторы ", id)).toBe(true);
    expect(hasExact(names, "штор", id)).toBe(false);
    expect(textKey("  Шторы   И  ")).toBe("шторы и");
  });
});

describe("theme", () => {
  it("parses and resolves", () => {
    expect(parseTheme("dark")).toBe("dark");
    expect(parseTheme("garbage")).toBe("system");
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
  });
});
