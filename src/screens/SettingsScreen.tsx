import { useCallback, useEffect, useState } from "react";
import { api, type BackupStatus } from "../api";
import { t } from "../i18n";
import { INTERVAL_OPTIONS, clampKeep, formatSize, formatTime, passwordProblem } from "../lib/settings";
import { useToasts } from "../ui/toasts";

export function SettingsScreen({ onBack }: { onBack: () => void }) {
  return (
    <div className="screen">
      <header className="toolbar">
        <button type="button" onClick={onBack}>
          {t.card.back}
        </button>
        <h1>{t.settings.title}</h1>
      </header>
      <main className="card-body">
        <PasswordSection />
        <BackupSection />
      </main>
    </div>
  );
}

function PasswordSection() {
  const toasts = useToasts();
  const [set, setSet] = useState<boolean | null>(null);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");

  const refresh = useCallback(() => {
    api
      .authStatus()
      .then((s) => setSet(s.passwordSet))
      .catch(toasts.error);
  }, [toasts.error]);

  useEffect(refresh, [refresh]);

  const problem = passwordProblem(next, repeat);
  const clear = () => {
    setCurrent("");
    setNext("");
    setRepeat("");
  };

  const save = async () => {
    try {
      await api.setPassword(current, next);
      toasts.show(set ? t.settings.passwordChanged : t.settings.passwordSet);
      clear();
      refresh();
    } catch (e) {
      toasts.error(e);
    }
  };

  const remove = async () => {
    try {
      await api.removePassword(current);
      toasts.show(t.settings.passwordRemoved);
      clear();
      refresh();
    } catch (e) {
      toasts.error(e);
    }
  };

  if (set === null) return null;
  return (
    <section>
      <h3>{t.settings.password}</h3>
      <p className="muted">{set ? t.settings.passwordOn : t.settings.passwordOff}</p>
      <p className="muted small-text">{t.settings.passwordNote}</p>
      <div className="settings-form">
        {set && (
          <input
            type="password"
            value={current}
            placeholder={t.settings.currentPassword}
            onChange={(e) => setCurrent(e.target.value)}
          />
        )}
        <input
          type="password"
          value={next}
          placeholder={t.settings.newPassword}
          onChange={(e) => setNext(e.target.value)}
        />
        <input
          type="password"
          value={repeat}
          placeholder={t.settings.repeatPassword}
          onChange={(e) => setRepeat(e.target.value)}
        />
        {next && problem === "mismatch" && <span className="warn-text">{t.settings.mismatch}</span>}
        <div className="row-buttons">
          <button
            type="button"
            className="primary"
            disabled={problem !== null || (set && !current)}
            onClick={save}
          >
            {set ? t.settings.changePassword : t.settings.setPassword}
          </button>
          {set && (
            <button type="button" disabled={!current} onClick={remove}>
              {t.settings.removePassword}
            </button>
          )}
        </div>
      </div>
    </section>
  );
}

function BackupSection() {
  const toasts = useToasts();
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(() => {
    api.backupStatus().then(setStatus).catch(toasts.error);
  }, [toasts.error]);

  useEffect(refresh, [refresh]);

  if (!status) return null;
  const { settings, state, copies } = status;

  const save = async (dir: string | null, interval: number, keep: number) => {
    try {
      await api.setBackupSettings(dir, interval, clampKeep(keep));
      refresh();
    } catch (e) {
      toasts.error(e);
    }
  };

  const chooseFolder = async () => {
    const dir = await api.pickFolder().catch((e) => {
      toasts.error(e);
      return null;
    });
    if (dir) await save(dir, settings.intervalMinutes, settings.keep);
  };

  const now = async () => {
    setBusy(true);
    try {
      await api.backupNow();
      toasts.show(t.settings.backupDone);
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
      refresh();
    }
  };

  const intervals = INTERVAL_OPTIONS.includes(settings.intervalMinutes)
    ? INTERVAL_OPTIONS
    : [...INTERVAL_OPTIONS, settings.intervalMinutes].sort((a, b) => a - b);

  return (
    <section>
      <h3>{t.settings.backup}</h3>
      <p className="muted small-text">{t.settings.backupNote}</p>
      <div className="settings-form">
        <div className="editor-row">
          <span className="grow path">{settings.dir ?? t.settings.backupOff}</span>
          <button type="button" onClick={chooseFolder}>
            {t.settings.chooseFolder}
          </button>
          {settings.dir && (
            <button type="button" onClick={() => save(null, settings.intervalMinutes, settings.keep)}>
              {t.settings.backupDisable}
            </button>
          )}
        </div>
        {settings.dir && (
          <>
            <label>
              {t.settings.every}{" "}
              <select
                value={settings.intervalMinutes}
                onChange={(e) => save(settings.dir, Number(e.target.value), settings.keep)}
              >
                {intervals.map((m) => (
                  <option key={m} value={m}>
                    {t.settings.interval(m)}
                  </option>
                ))}
              </select>{" "}
              {t.settings.ifChanged}
            </label>
            <label>
              {t.settings.keep}{" "}
              <input
                type="number"
                className="narrow"
                min={1}
                max={1000}
                defaultValue={settings.keep}
                key={settings.keep}
                onBlur={(e) => {
                  const keep = clampKeep(Number(e.target.value));
                  if (keep !== settings.keep) void save(settings.dir, settings.intervalMinutes, keep);
                }}
              />{" "}
              {t.settings.copies}
            </label>
            <div className="row-buttons">
              <button type="button" className="primary" disabled={busy} onClick={now}>
                {busy ? t.loading : t.settings.backupNow}
              </button>
              <button
                type="button"
                onClick={() => api.revealFile(copies[0]?.path ?? settings.dir ?? "").catch(toasts.error)}
              >
                {t.settings.showFolder}
              </button>
            </div>
            <p className="muted">
              {t.settings.lastBackup} {formatTime(state.lastBackupAt)}
            </p>
            {state.lastError && (
              <div className="banner warn">
                {t.settings.lastFailed} {t.errors[state.lastError] ?? state.lastError}
              </div>
            )}
            {copies.length > 0 && (
              <table className="rows copies">
                <tbody>
                  {copies.map((c) => (
                    <tr key={c.path}>
                      <td>{c.name}</td>
                      <td className="muted">{formatSize(c.size)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </>
        )}
      </div>
    </section>
  );
}
