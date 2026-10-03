import { useMemo, useState } from "react";
import {
  api,
  type Duplicate,
  type ImportAction,
  type ImportReport,
  type ImportRow,
  type Sheet,
} from "../api";
import { t } from "../i18n";
import {
  FIELDS,
  buildDrafts,
  detectHeaderRow,
  firstPhone,
  guessField,
  headerSignature,
  parseTemplates,
  upsertTemplate,
  type FieldId,
  type ImportDraft,
  type KindMode,
} from "../lib/importer";
import { useToasts } from "../ui/toasts";

const TEMPLATES_KEY = "importTemplates";
const PREVIEW_LIMIT = 300;

type Step = "file" | "map" | "preview" | "done";

interface PreviewRow {
  draft: ImportDraft;
  /** Совпадения того же вида — в них можно «дополнить». */
  duplicates: Duplicate[];
}

export function ImportScreen({ onBack, onImported }: { onBack: () => void; onImported: () => void }) {
  const toasts = useToasts();
  const [step, setStep] = useState<Step>("file");
  const [path, setPath] = useState("");
  const [sheets, setSheets] = useState<Sheet[]>([]);
  const [sheetIdx, setSheetIdx] = useState(0);
  const [headerRow, setHeaderRow] = useState(0);
  const [kindMode, setKindMode] = useState<KindMode>("company");
  const [mapping, setMapping] = useState<FieldId[]>([]);
  const [fromTemplate, setFromTemplate] = useState(false);
  const [remember, setRemember] = useState(true);
  const [preview, setPreview] = useState<PreviewRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);

  const sheet = sheets[sheetIdx];
  const headers = useMemo(() => sheet?.rows[headerRow] ?? [], [sheet, headerRow]);
  const width = useMemo(
    () =>
      Math.max(
        headers.length,
        ...(sheet?.rows.slice(headerRow + 1, headerRow + 50).map((r) => r.length) ?? [0]),
      ),
    [sheet, headerRow, headers],
  );

  /** Сопоставление для листа: сохранённый образец, иначе — угадать по заголовкам. */
  const initMapping = async (s: Sheet, row: number, mode: KindMode) => {
    const hdr = s.rows[row] ?? [];
    const templates = parseTemplates(await api.getSetting(TEMPLATES_KEY).catch(() => null));
    const tpl = templates.find((x) => x.signature === headerSignature(hdr));
    if (tpl) {
      setMapping(tpl.mapping);
      setKindMode(tpl.kindMode);
      setFromTemplate(true);
    } else {
      setMapping(hdr.map((h) => guessField(h, mode)));
      setFromTemplate(false);
    }
  };

  const chooseFile = async () => {
    try {
      const picked = await api.pickSpreadsheet();
      if (!picked) return;
      setBusy(true);
      const read = await api.readSpreadsheet(picked);
      const first = Math.max(
        0,
        read.findIndex((s) => s.rows.length > 0),
      );
      setPath(picked);
      setSheets(read);
      setSheetIdx(first);
      const row = detectHeaderRow(read[first]?.rows ?? []);
      setHeaderRow(row);
      if (read[first]) await initMapping(read[first], row, kindMode);
      setStep("map");
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
    }
  };

  const changeSheet = (idx: number) => {
    setSheetIdx(idx);
    const row = detectHeaderRow(sheets[idx].rows);
    setHeaderRow(row);
    void initMapping(sheets[idx], row, kindMode);
  };

  const changeHeaderRow = (row: number) => {
    setHeaderRow(row);
    void initMapping(sheet, row, kindMode);
  };

  const changeKindMode = (mode: KindMode) => {
    setKindMode(mode);
    // «Компания» угадывается по-разному для компаний и людей — пересчитать,
    // если человек ещё не правил сопоставление сам.
    if (!fromTemplate) setMapping(headers.map((h) => guessField(h, mode)));
  };

  const setField = (col: number, field: FieldId) => {
    const next = [...mapping];
    while (next.length <= col) next.push("skip");
    next[col] = field;
    setMapping(next);
    setFromTemplate(true); // ручная правка — больше не пересчитывать автоматически
  };

  const samples = (col: number) =>
    (sheet?.rows.slice(headerRow + 1) ?? [])
      .map((r) => r[col] ?? "")
      .filter((v) => v.trim())
      .slice(0, 3);

  const toPreview = async () => {
    setBusy(true);
    try {
      const drafts = buildDrafts(sheet.rows, headerRow, mapping, kindMode);
      const dups = await api.findDuplicatesBatch(
        drafts.map((d) => ({ title: d.title, phone: firstPhone(d) })),
      );
      setPreview(
        drafts.map((draft, i) => {
          const sameKind = (dups[i] ?? []).filter((x) => x.kind === draft.kind);
          // Дубль — показать и спросить; по умолчанию осторожно: пропустить.
          const action: ImportAction =
            draft.problems.includes("noTitle") || sameKind.length ? "skip" : "create";
          return { draft: { ...draft, action, mergeInto: sameKind[0]?.id ?? null }, duplicates: sameKind };
        }),
      );
      setStep("preview");
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
    }
  };

  const setAction = (i: number, action: ImportAction, mergeInto?: number) =>
    setPreview((rows) =>
      rows.map((r, j) =>
        j === i ? { ...r, draft: { ...r.draft, action, mergeInto: mergeInto ?? r.draft.mergeInto } } : r,
      ),
    );

  const setAllDuplicates = (action: ImportAction) =>
    setPreview((rows) =>
      rows.map((r) => (r.duplicates.length ? { ...r, draft: { ...r.draft, action } } : r)),
    );

  const runImport = async () => {
    setBusy(true);
    try {
      // Только поля строки импорта: номер строки и пометки — для экрана.
      const rows: ImportRow[] = preview.map(({ draft: d }) => ({
        kind: d.kind,
        title: d.title,
        contacts: d.contacts,
        addresses: d.addresses,
        roles: d.roles,
        specializations: d.specializations,
        note: d.note,
        company: d.company,
        position: d.position,
        action: d.action,
        mergeInto: d.action === "merge" ? d.mergeInto : null,
      }));
      const result = await api.importRows(rows);
      if (remember) {
        const templates = parseTemplates(await api.getSetting(TEMPLATES_KEY));
        const next = upsertTemplate(templates, { signature: headerSignature(headers), mapping, kindMode });
        await api.setSetting(TEMPLATES_KEY, JSON.stringify(next));
      }
      setReport(result);
      setStep("done");
      onImported();
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
    }
  };

  const counts = {
    create: preview.filter((r) => r.draft.action === "create").length,
    merge: preview.filter((r) => r.draft.action === "merge").length,
    skip: preview.filter((r) => r.draft.action === "skip").length,
    duplicates: preview.filter((r) => r.duplicates.length).length,
  };
  const hasTitle = mapping.includes("title");

  return (
    <div className="screen">
      <header className="toolbar">
        <button type="button" onClick={onBack}>
          {t.card.back}
        </button>
        <h1>{t.importer.title}</h1>
        <span className="muted steps">
          {(["file", "map", "preview", "done"] as const).map((s, i) => (
            <span key={s} className={s === step ? "step active" : "step"}>
              {i + 1}. {t.importer.steps[s]}
            </span>
          ))}
        </span>
      </header>

      <main className="import-body">
        {step === "file" && (
          <section>
            <p>{t.importer.fileHint}</p>
            <button type="button" className="primary" disabled={busy} onClick={chooseFile}>
              {busy ? t.loading : t.importer.chooseFile}
            </button>
          </section>
        )}

        {step === "map" && sheet && (
          <section>
            <p className="muted path">{path}</p>
            <div className="import-options">
              {sheets.length > 1 && (
                <label>
                  {t.importer.sheet}{" "}
                  <select value={sheetIdx} onChange={(e) => changeSheet(Number(e.target.value))}>
                    {sheets.map((s, i) => (
                      <option key={i} value={i}>
                        {s.name} ({s.rows.length})
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <label>
                {t.importer.headerRow}{" "}
                <input
                  type="number"
                  min={1}
                  max={Math.max(sheet.rows.length, 1)}
                  value={headerRow + 1}
                  onChange={(e) => {
                    const n = Number(e.target.value) - 1;
                    if (n >= 0 && n < sheet.rows.length) changeHeaderRow(n);
                  }}
                  className="narrow"
                />
              </label>
              <span>
                {t.importer.kindLabel}{" "}
                {(["company", "person", "column"] as const).map((m) => (
                  <label key={m} className="radio">
                    <input type="radio" checked={kindMode === m} onChange={() => changeKindMode(m)} />{" "}
                    {t.importer.kindModes[m]}
                  </label>
                ))}
              </span>
            </div>
            {sheet.truncated && <div className="banner warn">{t.importer.truncated}</div>}
            {fromTemplate && <p className="muted">{t.importer.templateApplied}</p>}

            <table className="rows mapping">
              <thead>
                <tr>
                  <th>{t.importer.column}</th>
                  <th>{t.importer.examples}</th>
                  <th>{t.importer.field}</th>
                </tr>
              </thead>
              <tbody>
                {Array.from({ length: width }, (_, col) => (
                  <tr key={col} className={(mapping[col] ?? "skip") === "skip" ? "unmapped" : undefined}>
                    <td className="cell-title">{headers[col] || t.importer.noHeader(col + 1)}</td>
                    <td className="muted">{samples(col).join(" · ")}</td>
                    <td>
                      <select
                        value={mapping[col] ?? "skip"}
                        onChange={(e) => setField(col, e.target.value as FieldId)}
                      >
                        {FIELDS.map((f) => (
                          <option key={f} value={f}>
                            {t.importer.fields[f]}
                          </option>
                        ))}
                      </select>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>

            <div className="import-actions">
              {!hasTitle && <span className="warn-text">{t.importer.needTitle}</span>}
              <button type="button" className="primary" disabled={!hasTitle || busy} onClick={toPreview}>
                {busy ? t.loading : t.importer.toPreview}
              </button>
            </div>
          </section>
        )}

        {step === "preview" && (
          <section>
            <p>
              {t.importer.summary(counts.create, counts.merge, counts.skip)}
              {preview.length > PREVIEW_LIMIT && (
                <span className="muted"> {t.importer.shownFirst(PREVIEW_LIMIT)}</span>
              )}
            </p>
            {counts.duplicates > 0 && (
              <div className="banner warn">
                {t.importer.duplicatesFound(counts.duplicates)}{" "}
                <button type="button" className="link" onClick={() => setAllDuplicates("skip")}>
                  {t.importer.allSkip}
                </button>{" "}
                <button type="button" className="link" onClick={() => setAllDuplicates("merge")}>
                  {t.importer.allMerge}
                </button>{" "}
                <button type="button" className="link" onClick={() => setAllDuplicates("create")}>
                  {t.importer.allCreate}
                </button>
              </div>
            )}
            <table className="rows preview">
              <thead>
                <tr>
                  <th>{t.importer.line}</th>
                  <th>{t.columns.title}</th>
                  <th>{t.columns.phone}</th>
                  <th>{t.importer.details}</th>
                  <th>{t.importer.action}</th>
                </tr>
              </thead>
              <tbody>
                {preview.slice(0, PREVIEW_LIMIT).map(({ draft, duplicates }, i) => (
                  <tr key={draft.line} className={draft.action === "skip" ? "unmapped" : undefined}>
                    <td className="muted">{draft.line}</td>
                    <td className="cell-title">
                      {draft.title || <span className="warn-text">{t.importer.problems.noTitle}</span>}
                      <div className="muted small-text">
                        {draft.kind === "company" ? t.card.company : t.card.person}
                        {draft.problems.includes("noKind") && ` — ${t.importer.problems.noKind}`}
                      </div>
                    </td>
                    <td>{draft.contacts.map((c) => c.value).join(", ")}</td>
                    <td className="small-text">
                      {[...draft.roles, ...draft.specializations, draft.company, ...draft.addresses]
                        .filter(Boolean)
                        .join(" · ")}
                    </td>
                    <td>
                      {draft.problems.includes("noTitle") ? (
                        <span className="muted">{t.importer.actions.skip}</span>
                      ) : (
                        <>
                          <select
                            value={draft.action}
                            onChange={(e) => setAction(i, e.target.value as ImportAction)}
                          >
                            <option value="create">{t.importer.actions.create}</option>
                            <option value="skip">{t.importer.actions.skip}</option>
                            {duplicates.length > 0 && (
                              <option value="merge">{t.importer.actions.merge}</option>
                            )}
                          </select>
                          {duplicates.length > 0 && (
                            <div className="small-text warn-text">
                              {t.importer.alreadyHave}{" "}
                              {draft.action === "merge" && duplicates.length > 1 ? (
                                <select
                                  value={draft.mergeInto ?? ""}
                                  onChange={(e) => setAction(i, "merge", Number(e.target.value))}
                                >
                                  {duplicates.map((d) => (
                                    <option key={d.id} value={d.id}>
                                      {d.title}
                                    </option>
                                  ))}
                                </select>
                              ) : (
                                duplicates.map((d) => d.title).join(", ")
                              )}
                            </div>
                          )}
                        </>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="import-actions">
              <label>
                <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} />{" "}
                {t.importer.remember}
              </label>
              <button type="button" onClick={() => setStep("map")}>
                {t.importer.backToMap}
              </button>
              <button
                type="button"
                className="primary"
                disabled={busy || counts.create + counts.merge === 0}
                onClick={runImport}
              >
                {busy ? t.loading : t.importer.run(counts.create + counts.merge)}
              </button>
            </div>
          </section>
        )}

        {step === "done" && report && (
          <section>
            <h2>{t.importer.doneTitle}</h2>
            <p>{t.importer.report(report)}</p>
            <button type="button" className="primary" onClick={onBack}>
              {t.importer.toList}
            </button>
          </section>
        )}
      </main>
    </div>
  );
}
