import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { addSample, formatMb, formatSpeed, percent, speed, type Sample } from "../lib/updateProgress";
import { useToasts } from "./toasts";

type State =
  | { kind: "idle" }
  | { kind: "downloading"; version: string; downloaded: number; total: number | null; speed: number | null }
  | { kind: "ready"; version: string }
  | { kind: "error" };

interface Progress {
  version: string;
  downloaded: number;
  total: number | null;
}

/** Строка обновления внизу окна: прогресс скачивания в фоне, затем
 * «установится при закрытии» и кнопка «Перезапустить сейчас». */
export function UpdateBar() {
  const toasts = useToasts();
  const [state, setState] = useState<State>({ kind: "idle" });
  const samples = useRef<Sample[]>([]);

  useEffect(() => {
    const unlisten = Promise.all([
      listen<Progress>("update-progress", (e) => {
        samples.current = addSample(samples.current, { at: Date.now(), bytes: e.payload.downloaded });
        setState({ kind: "downloading", ...e.payload, speed: speed(samples.current) });
      }),
      listen<string>("update-ready", (e) => setState({ kind: "ready", version: e.payload })),
      // Нет сети или GitHub недоступен — не повод мешать работе: тихая пометка.
      listen<string>("update-error", (e) => {
        console.warn("update check failed:", e.payload);
        setState({ kind: "error" });
      }),
    ]);
    // Подписка — раньше запуска проверки, чтобы не пропустить первые события.
    void unlisten.then(() => api.startUpdateCheck()).catch(() => undefined);
    return () => {
      void unlisten.then((fns) => fns.forEach((f) => f()));
    };
  }, []);

  if (state.kind === "idle") return null;
  if (state.kind === "error") return <span className="muted">{t.update.checkFailed}</span>;
  if (state.kind === "downloading") {
    const p = percent(state.downloaded, state.total);
    return (
      <span className="update-bar">
        {t.update.downloading(state.version)}{" "}
        <b>{p === null ? `${formatMb(state.downloaded)} МБ` : `${p}%`}</b>
        {state.total && ` (${formatMb(state.downloaded)} / ${formatMb(state.total)} МБ)`} ·{" "}
        {formatSpeed(state.speed)}
        {p !== null && (
          <span className="progress">
            <span style={{ width: `${p}%` }} />
          </span>
        )}
      </span>
    );
  }
  return (
    <span className="update-bar">
      {t.update.ready(state.version)}{" "}
      <button type="button" className="link" onClick={() => api.installUpdateNow().catch(toasts.error)}>
        {t.update.restartNow}
      </button>
    </span>
  );
}
