import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { errorCode } from "../api";
import { t } from "../i18n";

interface Toast {
  id: number;
  text: string;
  undo?: () => void;
  onExpire?: () => void;
}

interface ToastApi {
  show: (text: string, opts?: { undo?: () => void; onExpire?: () => void; timeoutMs?: number }) => void;
  error: (e: unknown) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

export const UNDO_WINDOW_MS = 10_000;
const PLAIN_MS = 3_000;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);
  const timers = useRef(new Map<number, number>());
  // Живые уведомления — ещё и здесь: действие по истечении вызывается вне
  // обновления состояния (React может вызвать обновление дважды).
  const live = useRef(new Map<number, Toast>());

  const dismiss = useCallback((id: number, expired: boolean) => {
    const toast = live.current.get(id);
    if (!toast) return;
    live.current.delete(id);
    window.clearTimeout(timers.current.get(id));
    timers.current.delete(id);
    if (expired) toast.onExpire?.();
    setToasts((list) => list.filter((x) => x.id !== id));
  }, []);

  const show = useCallback<ToastApi["show"]>(
    (text, opts = {}) => {
      const id = nextId.current++;
      const toast: Toast = { id, text, undo: opts.undo, onExpire: opts.onExpire };
      live.current.set(id, toast);
      setToasts((list) => [...list, toast]);
      const ms = opts.timeoutMs ?? (opts.undo ? UNDO_WINDOW_MS : PLAIN_MS);
      timers.current.set(
        id,
        window.setTimeout(() => dismiss(id, true), ms),
      );
    },
    [dismiss],
  );

  const error = useCallback<ToastApi["error"]>(
    (e) => {
      console.error(e);
      show(t.errors[errorCode(e)] ?? t.errors.unknown);
    },
    [show],
  );

  // Закрытие окна в окне «Отменить» — действие считается подтверждённым;
  // недостиранное подберёт очистка при следующем запуске.
  useEffect(() => {
    const map = timers.current;
    return () => map.forEach((timer) => window.clearTimeout(timer));
  }, []);

  return (
    <ToastContext.Provider value={{ show, error }}>
      {children}
      <div className="toasts" role="status">
        {toasts.map((toast) => (
          <div key={toast.id} className="toast">
            <span>{toast.text}</span>
            {toast.undo && (
              <button
                type="button"
                className="link"
                onClick={() => {
                  toast.undo?.();
                  dismiss(toast.id, false);
                }}
              >
                {t.toast.undo}
              </button>
            )}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToasts(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) throw new Error("useToasts outside ToastProvider");
  return api;
}
