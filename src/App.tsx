import { useCallback, useEffect, useState } from "react";
import { api, type CardKind } from "./api";
import { t } from "./i18n";
import { fetchAppInfo, showTestPlaque, type AppInfo } from "./lib/appInfo";
import { DEFAULT_COLUMNS, parseColumns, type ColumnSettings } from "./lib/columns";
import { NO_FILTERS, type SearchFilters } from "./lib/search";
import { DEFAULT_SORT, type SortState } from "./lib/sort";
import { parseTheme, resolveTheme, type ThemeChoice } from "./lib/theme";
import { CardScreen } from "./screens/CardScreen";
import { ExportScreen } from "./screens/ExportScreen";
import { ImportScreen } from "./screens/ImportScreen";
import { LoginScreen } from "./screens/LoginScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { ListScreen, type Tab } from "./screens/ListScreen";
import { ToastProvider, useToasts } from "./ui/toasts";

type Screen =
  | { name: "list" }
  | { name: "card"; id: number | null; newKind: CardKind }
  | { name: "import" }
  | { name: "export"; cardIds: number[]; peopleOnly: boolean }
  | { name: "settings" };

const COLUMNS_KEY = "listColumns";
const THEME_KEY = "theme";

export function App() {
  return (
    <ToastProvider>
      <Gate />
    </ToastProvider>
  );
}

/** Пока пароль задан и не введён — только экран входа: остальное даже не
 * запрашивает данные (Rust-часть их и не отдаст). */
function Gate() {
  const toasts = useToasts();
  const [unlocked, setUnlocked] = useState<boolean | null>(null);

  useEffect(() => {
    // До входа тема — как в Windows: сохранённый выбор лежит в базе.
    document.documentElement.dataset.theme = resolveTheme(
      "system",
      window.matchMedia("(prefers-color-scheme: dark)").matches,
    );
    api
      .authStatus()
      .then((s) => setUnlocked(s.unlocked))
      .catch(toasts.error);
  }, [toasts.error]);

  const onUnlocked = useCallback(
    (wasReset: boolean) => {
      if (wasReset) toasts.show(t.auth.wasReset);
      setUnlocked(true);
    },
    [toasts],
  );

  if (unlocked === null) return null;
  return unlocked ? <Shell /> : <LoginScreen onUnlocked={onUnlocked} />;
}

function Shell() {
  const toasts = useToasts();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [screen, setScreen] = useState<Screen>({ name: "list" });
  const [tab, setTab] = useState<Tab>("all");
  const [sort, setSort] = useState<SortState>(DEFAULT_SORT);
  const [columns, setColumns] = useState<ColumnSettings>(DEFAULT_COLUMNS);
  const [theme, setTheme] = useState<ThemeChoice>("system");
  const [revision, setRevision] = useState(0);
  // Поиск живёт здесь, а не в списке: вернулся из карточки — запрос и фильтры на месте.
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<SearchFilters>(NO_FILTERS);
  const [focusSearch, setFocusSearch] = useState(0);

  const changed = useCallback(() => setRevision((r) => r + 1), []);
  const searchFocused = useCallback(() => setFocusSearch(0), []);
  const back = useCallback(() => setScreen({ name: "list" }), []);
  const open = useCallback((id: number) => setScreen({ name: "card", id, newKind: "company" }), []);

  useEffect(() => {
    fetchAppInfo().then(setInfo).catch(toasts.error);
    api
      .getSetting(COLUMNS_KEY)
      .then((v) => setColumns(parseColumns(v)))
      .catch(toasts.error);
    api
      .getSetting(THEME_KEY)
      .then((v) => setTheme(parseTheme(v)))
      .catch(toasts.error);
  }, [toasts.error]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme = resolveTheme(theme, media.matches);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  // Esc: в поле ввода — выйти из поля (правка сохранится); иначе — назад к списку.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || screen.name !== "card") return;
      const el = document.activeElement;
      if (
        el instanceof HTMLInputElement ||
        el instanceof HTMLTextAreaElement ||
        el instanceof HTMLSelectElement
      ) {
        el.blur();
      } else {
        back();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [screen.name, back]);

  // Ctrl + F — наш поиск вместо встроенного поиска по странице. По коду
  // клавиши, а не по букве: в русской раскладке та же клавиша — «а».
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey && e.code === "KeyF")) return;
      e.preventDefault();
      back();
      setFocusSearch((n) => n + 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [back]);

  const saveColumns = (next: ColumnSettings) => {
    setColumns(next);
    api.setSetting(COLUMNS_KEY, JSON.stringify(next)).catch(toasts.error);
  };

  const saveTheme = (next: ThemeChoice) => {
    setTheme(next);
    api.setSetting(THEME_KEY, next).catch(toasts.error);
  };

  const deleted = (id: number, title: string) => {
    back();
    changed();
    toasts.show(t.toast.cardDeleted(title), {
      undo: () => api.restoreCard(id).then(changed).catch(toasts.error),
      onExpire: () => void api.purgeCard(id).catch(() => undefined),
    });
  };

  return (
    <div className="app">
      {showTestPlaque(info) && (
        <div className="test-plaque" title={`${t.testPlaqueHint}\n${info?.dbPath ?? ""}`}>
          {t.testPlaque}
        </div>
      )}
      {screen.name === "list" ? (
        <ListScreen
          tab={tab}
          onTab={setTab}
          sort={sort}
          onSort={setSort}
          columns={columns}
          onColumns={saveColumns}
          theme={theme}
          onTheme={saveTheme}
          revision={revision}
          onOpen={open}
          onCreate={(kind) => setScreen({ name: "card", id: null, newKind: kind })}
          onImport={() => setScreen({ name: "import" })}
          onSettings={() => setScreen({ name: "settings" })}
          onExport={(cardIds, peopleOnly) => setScreen({ name: "export", cardIds, peopleOnly })}
          query={query}
          onQuery={setQuery}
          filters={filters}
          onFilters={setFilters}
          focusSearch={focusSearch}
          onSearchFocused={searchFocused}
        />
      ) : screen.name === "import" ? (
        <ImportScreen onBack={back} onImported={changed} />
      ) : screen.name === "settings" ? (
        <SettingsScreen onBack={back} />
      ) : screen.name === "export" ? (
        <ExportScreen
          cardIds={screen.cardIds}
          kindMode={screen.peopleOnly ? "person" : "company"}
          onBack={back}
        />
      ) : (
        <CardScreen
          key={screen.id ?? `new-${screen.newKind}`}
          id={screen.id}
          newKind={screen.newKind}
          onBack={back}
          onOpen={open}
          onCreated={(id) => {
            changed();
            open(id);
          }}
          onDeleted={deleted}
          onChanged={changed}
        />
      )}
      {info && (
        <footer className="status">
          {t.info.version} {info.version}
        </footer>
      )}
    </div>
  );
}
