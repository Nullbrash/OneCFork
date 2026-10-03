import { describe, expect, it } from "vitest";
import {
  buildDrafts,
  detectHeaderRow,
  firstPhone,
  guessField,
  headerSignature,
  parseKind,
  parseTemplates,
  splitList,
  splitPhones,
  upsertTemplate,
  type FieldId,
} from "./importer";

describe("guessField", () => {
  it("recognizes common Russian headers", () => {
    expect(guessField("Телефон", "company")).toBe("phone");
    expect(guessField("Моб. тел.", "company")).toBe("phone");
    expect(guessField("WhatsApp", "company")).toBe("whatsapp");
    expect(guessField("Телеграм", "company")).toBe("telegram");
    expect(guessField("E-mail", "company")).toBe("email");
    expect(guessField("Адрес склада", "company")).toBe("address");
    expect(guessField("Услуги", "company")).toBe("specializations");
    expect(guessField("Примечание", "company")).toBe("note");
    expect(guessField("ФИО", "person")).toBe("title");
    expect(guessField("Должность", "person")).toBe("position");
    expect(guessField("что-то своё", "company")).toBe("skip");
  });

  it("«Компания» means title for companies and workplace for people", () => {
    expect(guessField("Компания", "company")).toBe("title");
    expect(guessField("Компания", "person")).toBe("company");
  });

  it("WhatsApp is not mistaken for a plain phone", () => {
    expect(guessField("Телефон WhatsApp", "company")).toBe("whatsapp");
  });
});

describe("parsing helpers", () => {
  it("detects the header row below a title line", () => {
    expect(detectHeaderRow([["Список поставщиков"], [], ["Название", "Телефон"], ["А", "1"]])).toBe(2);
    expect(detectHeaderRow([["одна"]])).toBe(0);
  });

  it("splits lists and phones", () => {
    expect(splitList("Шторы, карнизы; ткани")).toEqual(["Шторы", "карнизы", "ткани"]);
    expect(splitPhones("8 900 000-11-22, +7 900 000 33 44")).toEqual(["8 900 000-11-22", "+7 900 000 33 44"]);
    expect(splitPhones("8 900 000-11-22")).toEqual(["8 900 000-11-22"]);
  });

  it("parses kind words", () => {
    expect(parseKind("ООО")).toBe("company");
    expect(parseKind("Физ. лицо")).toBe("person");
    expect(parseKind("???")).toBeNull();
  });
});

describe("buildDrafts", () => {
  const rows = [
    ["Контакты"],
    ["Фамилия", "Имя", "Телефон", "Компания", "Специализация", "Комментарий"],
    [
      "Иванов",
      "Пётр",
      "8 900 000-11-22; 8 900 000-33-44",
      "Ателье Тест",
      "Шторы, карнизы",
      "звонить после 10",
    ],
    ["", "", "", "", "", ""],
    ["", "", "8 900 000-55-66", "", "", ""],
  ];
  const mapping: FieldId[] = ["title", "title", "phone", "company", "specializations", "note"];

  it("builds people with glued names, several phones and lists", () => {
    const drafts = buildDrafts(rows, 1, mapping, "person");
    expect(drafts).toHaveLength(2); // пустая строка пропущена
    const d = drafts[0];
    expect(d.line).toBe(3);
    expect(d.kind).toBe("person");
    expect(d.title).toBe("Иванов Пётр");
    expect(d.contacts.map((c) => c.value)).toEqual(["8 900 000-11-22", "8 900 000-33-44"]);
    expect(d.company).toBe("Ателье Тест");
    expect(d.specializations).toEqual(["Шторы", "карнизы"]);
    expect(d.note).toBe("звонить после 10");
    expect(d.problems).toEqual([]);
    expect(firstPhone(d)).toBe("8 900 000-11-22");
  });

  it("marks rows without a title", () => {
    expect(buildDrafts(rows, 1, mapping, "person")[1].problems).toContain("noTitle");
  });

  it("takes kind from a column when asked", () => {
    const drafts = buildDrafts(
      [
        ["Вид", "Название"],
        ["ИП", "Мастерская"],
        ["Частное лицо", "Иван"],
        ["?", "Непонятно"],
      ],
      0,
      ["kind", "title"],
      "column",
    );
    expect(drafts.map((d) => d.kind)).toEqual(["company", "person", "company"]);
    expect(drafts[2].problems).toContain("noKind");
  });
});

describe("templates", () => {
  it("signature ignores case and spaces; upsert replaces by signature", () => {
    expect(headerSignature([" Название ", "ТЕЛЕФОН"])).toBe(headerSignature(["название", "телефон"]));
    const a = { signature: "x", mapping: ["title"] as FieldId[], kindMode: "company" as const };
    const b = { ...a, kindMode: "person" as const };
    expect(upsertTemplate([a], b)).toEqual([b]);
  });

  it("drops broken saved templates", () => {
    const ok = { signature: "x", mapping: ["title"], kindMode: "company" };
    const json = JSON.stringify([ok, { signature: 1 }, { ...ok, mapping: ["bogus"] }]);
    expect(parseTemplates(json)).toHaveLength(1);
    expect(parseTemplates("{bad")).toEqual([]);
  });
});
