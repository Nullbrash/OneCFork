import { useMemo, useState } from "react";
import { api, type ExportField, type Sheet } from "../api";
import { t } from "../i18n";
import {
  EXPORT_FIELDS,
  defaultExportName,
  exportSignature,
  guessExportField,
  parseExportTemplates,
  upsertExportTemplate,
} from "../lib/exporter";
import { detectHeaderRow, type KindMode } from "../lib/importer";
import { useToasts } from "../ui/toasts";

const TEMPLATES_KEY = "exportTemplates";
const LAST_TEMPLATE_KEY = "lastExportTemplate";

type Step = "template" | "map" | "done";

interface Props {
  /** Карточки в порядке списка на экране (вкладка, поиск, фильтры, сортировка). */
  cardIds: number[];
  /** Для угадывания поля «Компания»: в списке только люди — это место работы. */
  kindMode: KindMode;
  onBack: () => void;
}

export function ExportScreen({ cardIds, kindMode, onBack }: Props) {
  const toasts = useToasts();
  const [step, setStep] = useState<Step>("template");
  const [hint, setHint] = useState(true);
  const [path, setPath] = useState("");
  const [sheets, setSheets] = useState<Sheet[]>([]);
  const [sheetIdx, setSheetIdx] = useState(0);
  const [headerRow, setHeaderRow] = useState(0);
  const [mapping, setMapping] = useState<(ExportField | null)[]>([]);
  const [fromTemplate, setFromTemplate] = useState(false);
  const [busy, setBusy] = useState(false);
  const [output, setOutput] = useState("");
  const [written, setWritten] = useState(0);

  const sheet = sheets[sheetIdx];
  const headers = useMemo(() => sheet?.rows[headerRow] ?? [], [sheet, headerRow]);

  const initMapping = async (s: Sheet, row: number) => {
    const hdr = s.rows[row] ?? [];
    const saved = parseExportTemplates(await api.getSetting(TEMPLATES_KEY).catch(() => null));
    const tpl = saved.find((x) => x.signature === exportSignature(hdr));
    if (tpl) {
      setMapping(tpl.mapping);
      setFromTemplate(true);
    } else {
      setMapping(hdr.map((h) => guessExportField(h, kindMode)));
      setFromTemplate(false);
    }
  };

  const loadTemplate = async (picked: string) => {
    setBusy(true);
    try {
      const read = await api.readSpreadsheet(picked);
      const first = Math.max(
        0,
        read.findIndex((s) => s.rows.length > 0),
      );
      const row = detectHeaderRow(read[first]?.rows ?? []);
      setPath(picked);
      setSheets(read);
      setSheetIdx(first);
      setHeaderRow(row);
      if (read[first]) await initMapping(read[first], row);
      setStep("map");
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
    }
  };

  const chooseTemplate = async () => {
    const picked = await api.pickExportTemplate().catch((e) => {
      toasts.error(e);
      return null;
    });
    if (picked) await loadTemplate(picked);
  };

  const useLast = async () => {
    const last = await api.getSetting(LAST_TEMPLATE_KEY).catch(() => null);
    if (last && (await api.fileExists(last))) await loadTemplate(last);
    else toasts.show(t.exporter.lastMissing);
  };

  const setField = (col: number, field: ExportField | null) => {
    const next = [...mapping];
    while (next.length <= col) next.push(null);
    next[col] = field;
    setMapping(next);
    setFromTemplate(true);
  };

  const run = async () => {
    const target = await api.pickSavePath(defaultExportName(path, new Date())).catch((e) => {
      toasts.error(e);
      return null;
    });
    if (!target) return;
    setBusy(true);
    try {
      const fullMapping = headers.map((_, i) => mapping[i] ?? null);
      const count = await api.exportCards({
        template: path,
        sheet: sheet.name,
        headerRow,
        mapping: fullMapping,
        cardIds,
        kindLabels: { company: t.card.company, person: t.card.person },
        output: target,
      });
      const saved = parseExportTemplates(await api.getSetting(TEMPLATES_KEY));
      await api.setSetting(
        TEMPLATES_KEY,
        JSON.stringify(
          upsertExportTemplate(saved, { signature: exportSignature(headers), mapping: fullMapping }),
        ),
      );
      await api.setSetting(LAST_TEMPLATE_KEY, path);
      setOutput(target);
      setWritten(count);
      setStep("done");
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
    }
  };

  const mappedCount = mapping.filter(Boolean).length;

  return (
    <div className="screen">
      <header className="toolbar">
        <button type="button" onClick={onBack}>
          {t.card.back}
        </button>
        <h1>{t.exporter.title}</h1>
        <span className="muted">{t.exporter.count(cardIds.length)}</span>
      </header>

      <main className="import-body">
        {step === "template" && (
          <section>
            <p>{t.exporter.intro}</p>
            <div className="import-actions left-actions">
              <button type="button" className="primary" disabled={busy} onClick={chooseTemplate}>
                {busy ? t.loading : t.exporter.chooseTemplate}
              </button>
              <button type="button" disabled={busy} onClick={useLast}>
                {t.exporter.useLast}
              </button>
              <div className="menu-anchor">
                <button type="button" onClick={() => setHint(!hint)} aria-expanded={hint}>
                  ⓘ {t.exporter.hintButton}
                </button>
                {hint && (
                  <div className="menu hint-popup" role="note">
                    <div className="menu-title">{t.exporter.hintTitle}</div>
                    <ul>
                      {t.exporter.hintRules.map((rule) => (
                        <li key={rule}>{rule}</li>
                      ))}
                    </ul>
                    <button type="button" className="link" onClick={() => setHint(false)}>
                      {t.exporter.hintClose}
                    </button>
                  </div>
                )}
              </div>
            </div>
          </section>
        )}

        {step === "map" && sheet && (
          <section>
            <p className="muted path">{path}</p>
            <div className="import-options">
              {sheets.length > 1 && (
                <label>
                  {t.importer.sheet}{" "}
                  <select
                    value={sheetIdx}
                    onChange={(e) => {
                      const idx = Number(e.target.value);
                      const row = detectHeaderRow(sheets[idx].rows);
                      setSheetIdx(idx);
                      setHeaderRow(row);
                      void initMapping(sheets[idx], row);
                    }}
                  >
                    {sheets.map((s, i) => (
                      <option key={i} value={i}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <label>
                {t.importer.headerRow}{" "}
                <input
                  type="number"
                  className="narrow"
                  min={1}
                  max={Math.max(sheet.rows.length, 1)}
                  value={headerRow + 1}
                  onChange={(e) => {
                    const n = Number(e.target.value) - 1;
                    if (n >= 0 && n < sheet.rows.length) {
                      setHeaderRow(n);
                      void initMapping(sheet, n);
                    }
                  }}
                />
              </label>
            </div>
            {fromTemplate && <p className="muted">{t.importer.templateApplied}</p>}
            {headers.length === 0 && <div className="banner warn">{t.exporter.noHeaders}</div>}

            <table className="rows mapping">
              <thead>
                <tr>
                  <th>{t.importer.column}</th>
                  <th>{t.exporter.fill}</th>
                </tr>
              </thead>
              <tbody>
                {headers.map((h, col) => (
                  <tr key={col} className={mapping[col] ? undefined : "unmapped"}>
                    <td className="cell-title">{h || t.importer.noHeader(col + 1)}</td>
                    <td>
                      <select
                        value={mapping[col] ?? ""}
                        onChange={(e) => setField(col, (e.target.value || null) as ExportField | null)}
                      >
                        <option value="">{t.exporter.leaveEmpty}</option>
                        {EXPORT_FIELDS.map((f) => (
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
            <p className="muted small-text">{t.exporter.multiColumnHint}</p>

            <div className="import-actions">
              <button type="button" onClick={() => setStep("template")}>
                {t.exporter.otherTemplate}
              </button>
              <button
                type="button"
                className="primary"
                disabled={busy || mappedCount === 0 || cardIds.length === 0}
                onClick={run}
              >
                {busy ? t.loading : t.exporter.run(cardIds.length)}
              </button>
            </div>
          </section>
        )}

        {step === "done" && (
          <section>
            <h2>{t.exporter.doneTitle}</h2>
            <p>{t.exporter.done(written)}</p>
            <p className="muted path">{output}</p>
            <div className="import-actions left-actions">
              <button
                type="button"
                className="primary"
                onClick={() => api.openFileLink("local", output).catch(toasts.error)}
              >
                {t.exporter.openFile}
              </button>
              <button type="button" onClick={() => api.revealFile(output).catch(toasts.error)}>
                {t.exporter.showInFolder}
              </button>
              <button type="button" onClick={onBack}>
                {t.importer.toList}
              </button>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}
