import { describe, expect, it } from "vitest";
import {
  defaultExportName,
  exportSignature,
  guessExportField,
  parseExportTemplates,
  upsertExportTemplate,
} from "./exporter";

describe("guessExportField", () => {
  it("ignores trailing numbers so Телефон 1 / Телефон 2 both map to phone", () => {
    expect(guessExportField("Телефон 1", "company")).toBe("phone");
    expect(guessExportField("Телефон №2", "company")).toBe("phone");
    expect(guessExportField("Тел-3", "company")).toBe("phone");
  });

  it("maps known headers and leaves unknown ones empty", () => {
    expect(guessExportField("Наименование", "company")).toBe("title");
    expect(guessExportField("Компания", "person")).toBe("company");
    expect(guessExportField("E-mail", "company")).toBe("email");
    expect(guessExportField("Мой столбец", "company")).toBeNull();
  });
});

describe("export templates", () => {
  it("round-trips and replaces by signature, dropping broken entries", () => {
    const a = {
      signature: exportSignature(["Имя", "Телефон 1"]),
      mapping: ["title" as const, "phone" as const],
    };
    const b = { ...a, mapping: ["title" as const, null] };
    expect(upsertExportTemplate([a], b)).toEqual([b]);
    const json = JSON.stringify([a, { signature: 1 }, { signature: "x", mapping: ["bogus"] }]);
    expect(parseExportTemplates(json)).toEqual([a]);
    expect(parseExportTemplates("nope")).toEqual([]);
  });

  it("builds a default output name next to the template name", () => {
    expect(defaultExportName("C:\\Отчёты\\Поставщики.xlsx", new Date("2026-10-03T12:00:00Z"))).toBe(
      "Поставщики — выгрузка 2026-10-03.xlsx",
    );
  });
});
