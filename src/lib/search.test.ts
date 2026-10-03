import { describe, expect, it } from "vitest";
import type { ListRow } from "../api";
import {
  NO_FILTERS,
  buildIndex,
  editDistance,
  isActive,
  layoutVariants,
  normalize,
  phoneDigits,
  searchRows,
} from "./search";

const row = (id: number, title: string, extra: Partial<ListRow> = {}): ListRow => ({
  id,
  kind: "company",
  title,
  roles: [],
  specializations: [],
  companies: [],
  mainPhone: null,
  roleIds: [],
  specializationIds: [],
  addressLabelIds: [],
  searchText: "",
  phoneKeys: [],
  ...extra,
});

const ROLE_INSTALLER = 2;
const ROLE_COURIER = 3;
const SPEC_CURTAINS = 10;
const LABEL_STORE = 20;

const rows: ListRow[] = [
  row(1, "Ателье Тест", {
    roles: ["Поставщик"],
    specializations: ["Шторы"],
    specializationIds: [SPEC_CURTAINS],
    searchText: "ул. Складская, 5\n@atelier_test\nзвонить после 10",
    addressLabelIds: [LABEL_STORE],
    phoneKeys: ["9000001122"],
  }),
  row(2, "Иван Монтажников", {
    kind: "person",
    roles: ["Монтажник"],
    roleIds: [ROLE_INSTALLER],
    specializations: ["Шторы"],
    specializationIds: [SPEC_CURTAINS],
    companies: ["Ателье Тест"],
  }),
  row(3, "Фирма Ёлка", { kind: "company", roles: ["Доставщик"], roleIds: [ROLE_COURIER] }),
];
const index = buildIndex(rows);
const ids = (q: string, f = NO_FILTERS) => searchRows(index, q, f).map((r) => r.id);

describe("text search", () => {
  it("finds by part of any field, case-insensitively", () => {
    expect(ids("ател")).toEqual([1, 2]);
    expect(ids("СКЛАДСК")).toEqual([1]);
    expect(ids("@atelier")).toEqual([1]);
  });

  it("finds people by their company name (user decision)", () => {
    expect(ids("ателье")).toContain(2);
  });

  it("requires every word, in any order", () => {
    expect(ids("шторы иван")).toEqual([2]);
    expect(ids("иван склад")).toEqual([]);
  });

  it("treats ё as е", () => {
    expect(ids("елка")).toEqual([3]);
    expect(ids("ёлка")).toEqual([3]);
  });

  it("forgives typos in longer words", () => {
    expect(ids("монтжник")).toEqual([2]);
    expect(ids("монтжн")).toEqual([2]);
    expect(ids("шиоры")).toEqual([1, 2]);
  });

  it("does not forgive typos in short words", () => {
    expect(ids("ива")).toEqual([2]);
    expect(ids("ифа")).toEqual([]);
  });

  it("understands shifted symbols of the wrong layout (capital Б is '<')", () => {
    const idx = buildIndex([row(7, "Босс"), row(8, "Хозяин"), row(9, "Жук")]);
    const found = (q: string) => searchRows(idx, q, NO_FILTERS).map((r) => r.id);
    expect(found("<jc")).toEqual([7]);
    expect(found("<jcc")).toEqual([7]);
    expect(found("{jp")).toEqual([8]);
    expect(found(":er")).toEqual([9]);
  });

  it("understands the wrong keyboard layout", () => {
    expect(ids("infnmt")).toEqual([]); // нет такого слова — и не должно быть ложных срабатываний
    expect(ids("injhs")).toEqual([1, 2]); // «шторы» в английской раскладке
  });

  it("finds phones in any format", () => {
    expect(ids("8 900 000-11-22")).toEqual([1]);
    expect(ids("+7(900)0001122")).toEqual([1]);
    expect(ids("1122")).toEqual([1]);
  });

  it("empty query returns everything", () => {
    expect(ids("   ")).toEqual([1, 2, 3]);
  });
});

describe("filters", () => {
  it("OR inside one filter", () => {
    expect(ids("", { ...NO_FILTERS, roleIds: [ROLE_INSTALLER, ROLE_COURIER] })).toEqual([2, 3]);
  });

  it("AND between filters and with text", () => {
    const f = { ...NO_FILTERS, roleIds: [ROLE_INSTALLER, ROLE_COURIER], specializationIds: [SPEC_CURTAINS] };
    expect(ids("", f)).toEqual([2]);
    expect(ids("елка", f)).toEqual([]);
    expect(ids("", { ...NO_FILTERS, addressLabelIds: [LABEL_STORE] })).toEqual([1]);
  });

  it("knows when search is active", () => {
    expect(isActive("", NO_FILTERS)).toBe(false);
    expect(isActive("x", NO_FILTERS)).toBe(true);
    expect(isActive("", { ...NO_FILTERS, roleIds: [1] })).toBe(true);
  });
});

describe("helpers", () => {
  it("layout variants and phone digits", () => {
    expect(layoutVariants("injhs")).toContain("шторы");
    expect(layoutVariants("ыфьыгтп")).toContain("samsung");
    expect(phoneDigits("8 (900) 123-45-67")).toBe("9001234567");
    expect(phoneDigits("ул 5")).toBeNull();
    expect(phoneDigits("12")).toBeNull();
  });

  it("edit distance with early exit", () => {
    expect(editDistance("штора", "шторы", 1)).toBe(1);
    expect(editDistance("abc", "xyzxyz", 1)).toBe(2);
    expect(normalize("  Ёж   Ёлкин ")).toBe("еж елкин");
  });
});
