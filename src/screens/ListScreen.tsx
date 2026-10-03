import { useEffect, useState } from "react";
import { api, type CardKind, type ListRow } from "../api";
import { t } from "../i18n";
import {
  ALL_COLUMNS,
  LOCKED_COLUMN,
  cellText,
  moveColumn,
  toggleColumn,
  visibleColumns,
  type ColumnId,
  type ColumnSettings,
} from "../lib/columns";
import { nextSort, sortRows, type SortState } from "../lib/sort";
import type { ThemeChoice } from "../lib/theme";
import { useToasts } from "../ui/toasts";

export type Tab = "all" | "companies" | "people";

interface Props {
  tab: Tab;
  onTab: (tab: Tab) => void;
  sort: SortState;
  onSort: (sort: SortState) => void;
  columns: ColumnSettings;
  onColumns: (columns: ColumnSettings) => void;
  theme: ThemeChoice;
  onTheme: (theme: ThemeChoice) => void;
  /** Меняется после правок — список перечитывается. */
  revision: number;
  onOpen: (id: number) => void;
  onCreate: (kind: CardKind) => void;
}

export function ListScreen(props: Props) {
  const { tab, revision } = props;
  const toasts = useToasts();
  const [rows, setRows] = useState<ListRow[] | null>(null);
  const [menu, setMenu] = useState<"new" | "columns" | "settings" | null>(null);

  useEffect(() => {
    api.listRows(null).then(setRows).catch(toasts.error);
  }, [revision, toasts.error]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [menu]);

  const companies = rows?.filter((r) => r.kind === "company") ?? [];
  const people = rows?.filter((r) => r.kind === "person") ?? [];

  return (
    <div className="screen">
      <header className="toolbar">
        <h1>{t.appTitle}</h1>
        <nav className="tabs">
          {(["all", "companies", "people"] as const).map((id) => (
            <button
              key={id}
              type="button"
              className={tab === id ? "tab active" : "tab"}
              onClick={() => props.onTab(id)}
            >
              {t.tabs[id]}
            </button>
          ))}
        </nav>
        <div className="toolbar-actions" onClick={(e) => e.stopPropagation()}>
          <div className="menu-anchor">
            <button type="button" className="primary" onClick={() => setMenu(menu === "new" ? null : "new")}>
              {t.list.newCard}
            </button>
            {menu === "new" && (
              <div className="menu">
                <button type="button" onClick={() => props.onCreate("company")}>
                  {t.list.newCompany}
                </button>
                <button type="button" onClick={() => props.onCreate("person")}>
                  {t.list.newPerson}
                </button>
              </div>
            )}
          </div>
          <div className="menu-anchor">
            <button type="button" onClick={() => setMenu(menu === "columns" ? null : "columns")}>
              {t.list.columns}
            </button>
            {menu === "columns" && <ColumnsMenu columns={props.columns} onColumns={props.onColumns} />}
          </div>
          <div className="menu-anchor">
            <button
              type="button"
              title={t.list.settings}
              aria-label={t.list.settings}
              onClick={() => setMenu(menu === "settings" ? null : "settings")}
            >
              ⚙
            </button>
            {menu === "settings" && (
              <div className="menu">
                <div className="menu-title">{t.theme.label}</div>
                {(["system", "light", "dark"] as const).map((choice) => (
                  <label key={choice} className="menu-option">
                    <input
                      type="radio"
                      name="theme"
                      checked={props.theme === choice}
                      onChange={() => props.onTheme(choice)}
                    />
                    {t.theme[choice]}
                  </label>
                ))}
              </div>
            )}
          </div>
        </div>
      </header>

      <main className="list-body">
        {rows === null && <p className="muted">{t.loading}</p>}
        {rows !== null && rows.length === 0 && <p className="empty">{t.list.empty}</p>}
        {rows !== null && rows.length > 0 && (
          <>
            {tab !== "people" && (
              <RowsSection
                title={tab === "all" ? t.sections.companies : null}
                rows={companies}
                forCompanies
                {...props}
              />
            )}
            {tab !== "companies" && (
              <RowsSection
                title={tab === "all" ? t.sections.people : null}
                rows={people}
                forCompanies={false}
                {...props}
              />
            )}
          </>
        )}
      </main>
    </div>
  );
}

function RowsSection({
  title,
  rows,
  forCompanies,
  columns,
  sort,
  onSort,
  onOpen,
}: Props & { title: string | null; rows: ListRow[]; forCompanies: boolean }) {
  const toasts = useToasts();
  const visible = visibleColumns(columns, forCompanies);
  const sorted = sortRows(rows, visible.includes(sort.column) ? sort : { column: "title", dir: sort.dir });

  const copy = (text: string) =>
    api
      .copyText(text)
      .then(() => toasts.show(t.toast.copied))
      .catch(toasts.error);

  return (
    <section className="rows-section">
      {title && <h2>{title}</h2>}
      {rows.length === 0 ? (
        <p className="muted">{t.list.emptySection}</p>
      ) : (
        <table className="rows">
          <thead>
            <tr>
              {visible.map((c) => (
                <th key={c} onClick={() => onSort(nextSort(sort, c))} className="sortable">
                  {t.columns[c]}
                  {sort.column === c && <span className="sort-mark">{sort.dir === "asc" ? " ▲" : " ▼"}</span>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sorted.map((row) => (
              <tr key={row.id} onClick={() => onOpen(row.id)}>
                {visible.map((c) => (
                  <td key={c} className={c === "title" ? "cell-title" : undefined}>
                    {c === "phone" && row.mainPhone ? (
                      <button
                        type="button"
                        className="phone-cell"
                        title={t.list.copyPhone}
                        onClick={(e) => {
                          e.stopPropagation();
                          void copy(row.mainPhone ?? "");
                        }}
                      >
                        ☎ {row.mainPhone}
                      </button>
                    ) : (
                      cellText(row, c)
                    )}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function ColumnsMenu({
  columns,
  onColumns,
}: {
  columns: ColumnSettings;
  onColumns: (columns: ColumnSettings) => void;
}) {
  const ordered: ColumnId[] = columns.order.filter((c) => ALL_COLUMNS.includes(c));
  return (
    <div className="menu columns-menu">
      {ordered.map((c, i) => (
        <div key={c} className="menu-option">
          <label>
            <input
              type="checkbox"
              checked={!columns.hidden.includes(c)}
              disabled={c === LOCKED_COLUMN}
              onChange={() => onColumns(toggleColumn(columns, c))}
            />
            {t.columns[c]}
          </label>
          <span className="order-buttons">
            <button
              type="button"
              disabled={i === 0}
              title={t.list.moveUp}
              onClick={() => onColumns(moveColumn(columns, c, -1))}
            >
              ↑
            </button>
            <button
              type="button"
              disabled={i === ordered.length - 1}
              title={t.list.moveDown}
              onClick={() => onColumns(moveColumn(columns, c, 1))}
            >
              ↓
            </button>
          </span>
        </div>
      ))}
    </div>
  );
}
