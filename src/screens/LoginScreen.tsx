import { useEffect, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { useToasts } from "../ui/toasts";

/** Проверка, не удалён ли файл пароля (сброс), — пока открыт экран входа. */
const RESET_CHECK_MS = 2000;

export function LoginScreen({ onUnlocked }: { onUnlocked: (wasReset: boolean) => void }) {
  const toasts = useToasts();
  const [password, setPassword] = useState("");
  const [wrong, setWrong] = useState(false);
  const [busy, setBusy] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [resetShown, setResetShown] = useState(false);

  // Сброс по решению пользователя: удалил файл пароля — программа пускает
  // и предлагает задать новый.
  useEffect(() => {
    const timer = window.setInterval(() => {
      api
        .authStatus()
        .then((s) => !s.passwordSet && onUnlocked(true))
        .catch(() => undefined);
    }, RESET_CHECK_MS);
    return () => window.clearInterval(timer);
  }, [onUnlocked]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [menu]);

  const submit = async () => {
    if (!password || busy) return;
    setBusy(true);
    try {
      if (await api.unlock(password)) onUnlocked(false);
      else {
        setWrong(true);
        setPassword("");
      }
    } catch (e) {
      toasts.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="login"
      onContextMenu={(e) => {
        // Своё меню правой кнопки — единственный путь к скрытой кнопке сброса.
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      <form
        className="login-box"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <h1>{t.appTitle}</h1>
        <input
          type="password"
          autoFocus
          value={password}
          placeholder={t.auth.password}
          onChange={(e) => {
            setPassword(e.target.value);
            setWrong(false);
          }}
        />
        {wrong && <p className="error">{t.auth.wrong}</p>}
        <button type="submit" className="primary" disabled={!password || busy}>
          {busy ? t.loading : t.auth.enter}
        </button>
        {resetShown && (
          <div className="reset-box">
            <p className="muted small-text">{t.auth.resetHint}</p>
            <button type="button" onClick={() => api.revealPasswordFile().catch(toasts.error)}>
              {t.auth.resetButton}
            </button>
          </div>
        )}
      </form>
      {menu && (
        <div className="menu context-menu" style={{ left: menu.x, top: menu.y }}>
          <button type="button" onClick={() => setResetShown(true)}>
            {t.auth.showButton}
          </button>
        </div>
      )}
    </div>
  );
}
