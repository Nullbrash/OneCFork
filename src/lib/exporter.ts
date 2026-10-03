import type { ExportField } from "../api";
import { guessField, type KindMode } from "./importer";
import { textKey } from "./match";

export const EXPORT_FIELDS: readonly ExportField[] = [
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

/**
 * Поле по заголовку образца. Номер в конце («Телефон 2») не мешает: те же
 * правила, что у импорта, — столбцы с одним полем заполняются по порядку.
 */
export function guessExportField(header: string, kindMode: KindMode): ExportField | null {
  const base = header.replace(/[\s№#-]*\d+\s*$/, "");
  const f = guessField(base, kindMode);
  return f === "skip" ? null : f;
}

export interface ExportTemplate {
  signature: string;
  mapping: (ExportField | null)[];
}

export function exportSignature(headers: readonly string[]): string {
  return headers.map(textKey).join("|");
}

export function parseExportTemplates(json: string | null): ExportTemplate[] {
  if (!json) return [];
  try {
    const raw: unknown = JSON.parse(json);
    if (!Array.isArray(raw)) return [];
    return raw.filter(
      (t): t is ExportTemplate =>
        typeof t === "object" &&
        t !== null &&
        typeof t.signature === "string" &&
        Array.isArray(t.mapping) &&
        t.mapping.every((f: unknown) => f === null || EXPORT_FIELDS.includes(f as ExportField)),
    );
  } catch {
    return [];
  }
}

export function upsertExportTemplate(list: readonly ExportTemplate[], tpl: ExportTemplate): ExportTemplate[] {
  return [...list.filter((t) => t.signature !== tpl.signature), tpl];
}

/** Имя файла по умолчанию: «образец — выгрузка 2026-10-03.xlsx». */
export function defaultExportName(templatePath: string, today: Date): string {
  const file = templatePath.split(/[\\/]/).pop() ?? "export.xlsx";
  const base = file.replace(/\.xlsx$/i, "");
  const date = today.toISOString().slice(0, 10);
  return `${base} — выгрузка ${date}.xlsx`;
}
