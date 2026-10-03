import type { CardKind, Channel, ImportRow } from "../api";
import { textKey } from "./match";

export type FieldId =
  | "skip"
  | "title"
  | "kind"
  | "phone"
  | "telegram"
  | "whatsapp"
  | "viber"
  | "max"
  | "email"
  | "address"
  | "roles"
  | "specializations"
  | "note"
  | "company"
  | "position";

export const FIELDS: readonly FieldId[] = [
  "skip",
  "title",
  "kind",
  "phone",
  "telegram",
  "whatsapp",
  "viber",
  "max",
  "email",
  "address",
  "roles",
  "specializations",
  "note",
  "company",
  "position",
];

const CHANNEL_FIELDS: Partial<Record<FieldId, Channel>> = {
  phone: "phone",
  telegram: "telegram",
  whatsapp: "whatsapp",
  viber: "viber",
  max: "max",
  email: "email",
};

/** Все строки — компании, все — люди, или вид берётся из столбца «Вид». */
export type KindMode = "company" | "person" | "column";

/**
 * Угадать поле по заголовку столбца. «Компания» в таблице людей — место
 * работы, а в таблице компаний — само название.
 */
export function guessField(header: string, kindMode: KindMode): FieldId {
  const h = textKey(header);
  if (!h) return "skip";
  const has = (...words: string[]) => words.some((w) => h.includes(w));
  if (has("whatsapp", "ватсап", "вацап", "вотсап")) return "whatsapp";
  if (has("telegram", "телеграм", "телега") || h === "тг" || h === "tg") return "telegram";
  if (has("viber", "вайбер")) return "viber";
  if (h === "max" || h === "макс") return "max";
  if (has("e-mail", "email", "почта", "mail")) return "email";
  if (has("тел", "моб", "сот", "phone", "номер")) return "phone";
  if (has("адрес", "address")) return "address";
  if (has("должн", "position")) return "position";
  if (has("специализ", "услуг", "вид работ", "направлен", "профил")) return "specializations";
  if (has("роль", "роли", "категор")) return "roles";
  if (has("примеч", "заметк", "коммент", "описан")) return "note";
  if (h === "вид" || h === "тип") return "kind";
  if (has("компан", "организац", "фирм")) return kindMode === "person" ? "company" : "title";
  if (has("назван", "наименов", "фио", "имя", "контакт", "name")) return "title";
  return "skip";
}

/** Строка заголовков — первая из первых 20, где заполнено хотя бы 2 ячейки. */
export function detectHeaderRow(rows: readonly string[][]): number {
  const limit = Math.min(rows.length, 20);
  for (let i = 0; i < limit; i++) {
    if (rows[i].filter((c) => c.trim()).length >= 2) return i;
  }
  return 0;
}

/** Список в одной ячейке: «Шторы, карнизы; ткани». */
export function splitList(value: string): string[] {
  return value
    .split(/[,;\n]/)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Несколько номеров в ячейке: по запятой, «;», «/», переводу строки — но не
 * по пробелу: пробелы бывают внутри одного номера. */
export function splitPhones(value: string): string[] {
  return value
    .split(/[,;/\n]/)
    .map((s) => s.trim())
    .filter(Boolean);
}

export function parseKind(value: string): CardKind | null {
  const v = textKey(value);
  if (!v) return null;
  if (/(компан|организ|фирм|ооо|оао|зао|ао|ип|юр)/.test(v)) return "company";
  if (/(человек|физ|частн|лицо|сотрудник|работник)/.test(v)) return "person";
  return null;
}

export interface ImportDraft extends ImportRow {
  /** Номер строки в Excel (с 1) — чтобы человек нашёл её в своей таблице. */
  line: number;
  problems: ("noTitle" | "noKind")[];
}

export function buildDrafts(
  rows: readonly string[][],
  headerRow: number,
  mapping: readonly FieldId[],
  kindMode: KindMode,
): ImportDraft[] {
  const drafts: ImportDraft[] = [];
  for (let i = headerRow + 1; i < rows.length; i++) {
    const cells = rows[i];
    if (!cells.some((c) => c.trim())) continue;
    const draft: ImportDraft = {
      line: i + 1,
      kind: kindMode === "person" ? "person" : "company",
      title: "",
      contacts: [],
      addresses: [],
      roles: [],
      specializations: [],
      note: "",
      company: "",
      position: "",
      action: "create",
      mergeInto: null,
      problems: [],
    };
    let kindFromColumn: CardKind | null = null;
    const titles: string[] = [];
    const notes: string[] = [];
    mapping.forEach((field, col) => {
      const value = (cells[col] ?? "").trim();
      if (!value || field === "skip") return;
      const channel = CHANNEL_FIELDS[field];
      if (channel) {
        const values = channel === "email" ? splitList(value) : splitPhones(value);
        for (const v of values) draft.contacts.push({ channel, value: v });
        return;
      }
      switch (field) {
        case "title":
          titles.push(value);
          break;
        case "kind":
          kindFromColumn = parseKind(value);
          break;
        case "address":
          draft.addresses.push(value);
          break;
        case "roles":
          draft.roles.push(...splitList(value));
          break;
        case "specializations":
          draft.specializations.push(...splitList(value));
          break;
        case "note":
          notes.push(value);
          break;
        case "company":
          draft.company = value;
          break;
        case "position":
          draft.position = value;
          break;
      }
    });
    // Несколько столбцов «Название» (Фамилия + Имя) — склеиваются через пробел.
    draft.title = titles.join(" ");
    draft.note = notes.join("\n");
    if (kindMode === "column") {
      if (kindFromColumn) draft.kind = kindFromColumn;
      else draft.problems.push("noKind");
    }
    if (!draft.title) draft.problems.push("noTitle");
    drafts.push(draft);
  }
  return drafts;
}

/** Первый номер строки — для поиска дублей по телефону. */
export function firstPhone(d: ImportRow): string | null {
  return (
    d.contacts.find((c) => c.channel === "phone" || c.channel === "whatsapp" || c.channel === "viber")
      ?.value ?? null
  );
}

export interface ImportTemplate {
  signature: string;
  mapping: FieldId[];
  kindMode: KindMode;
}

/** Подпись таблицы — её заголовки; одинаковые заголовки — тот же образец. */
export function headerSignature(headers: readonly string[]): string {
  return headers.map(textKey).join("|");
}

export function parseTemplates(json: string | null): ImportTemplate[] {
  if (!json) return [];
  try {
    const raw: unknown = JSON.parse(json);
    if (!Array.isArray(raw)) return [];
    return raw.filter(
      (t): t is ImportTemplate =>
        typeof t === "object" &&
        t !== null &&
        typeof t.signature === "string" &&
        Array.isArray(t.mapping) &&
        t.mapping.every((f: unknown) => FIELDS.includes(f as FieldId)) &&
        ["company", "person", "column"].includes(t.kindMode),
    );
  } catch {
    return [];
  }
}

export function upsertTemplate(list: readonly ImportTemplate[], tpl: ImportTemplate): ImportTemplate[] {
  return [...list.filter((t) => t.signature !== tpl.signature), tpl];
}
