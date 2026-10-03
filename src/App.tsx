import { useEffect, useState } from "react";
import { t } from "./i18n";
import { fetchAppInfo, showTestPlaque, type AppInfo } from "./lib/appInfo";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    fetchAppInfo()
      .then(setInfo)
      .catch(() => setFailed(true));
  }, []);

  return (
    <div className="app">
      {showTestPlaque(info) && (
        <div className="test-plaque" title={t.testPlaqueHint}>
          {t.testPlaque}
        </div>
      )}
      <main className="stub">
        <h1>{t.appTitle}</h1>
        <p>{t.stubNotice}</p>
        {failed && <p className="error">{t.loadError}</p>}
        {!info && !failed && <p>{t.loading}</p>}
        {info && (
          <dl>
            <dt>{t.info.version}</dt>
            <dd>{info.version}</dd>
            <dt>{t.info.database}</dt>
            <dd>{info.dbPath}</dd>
            <dt>{t.info.schema}</dt>
            <dd>{info.schemaVersion}</dd>
          </dl>
        )}
      </main>
    </div>
  );
}
