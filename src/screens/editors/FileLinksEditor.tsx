import { useEffect, useState } from "react";
import { api, type CardDetails, type FileLink, type FileLinkKind } from "../../api";
import { t } from "../../i18n";
import { AutoInput } from "../../ui/AutoInput";
import { useToasts } from "../../ui/toasts";
import type { Run } from "../CardScreen";

const KINDS: FileLinkKind[] = ["local", "yandex_disk"];

export function FileLinksEditor({ details, run }: { details: CardDetails; run: Run }) {
  const toasts = useToasts();
  const [missing, setMissing] = useState<Set<number>>(new Set());
  const [kind, setKind] = useState<FileLinkKind>("local");
  const [target, setTarget] = useState("");
  const [title, setTitle] = useState("");
  const cardId = details.card.id;

  useEffect(() => {
    // Файл на ПК могли переименовать или перенести — проверить при открытии карточки.
    const local = details.fileLinks.filter((f) => f.kind === "local");
    Promise.all(local.map(async (f) => [f.id, await api.fileExists(f.target)] as const))
      .then((checks) => setMissing(new Set(checks.filter(([, ok]) => !ok).map(([id]) => id))))
      .catch(toasts.error);
  }, [details, toasts.error]);

  const relocate = async (f: FileLink) => {
    const path = await api.pickFile().catch((e) => {
      toasts.error(e);
      return null;
    });
    if (path) void run(() => api.updateFileLink(f.id, f.kind, path, f.title));
  };

  const remove = (f: FileLink) =>
    run(async () => {
      await api.removeFileLink(f.id);
      toasts.show(t.toast.itemRemoved, {
        undo: () => void run(() => api.addFileLink(cardId, f.kind, f.target, f.title)),
      });
    });

  const add = () => {
    if (!target.trim()) return;
    const [k, v, n] = [kind, target, title];
    setTarget("");
    setTitle("");
    void run(() => api.addFileLink(cardId, k, v, n));
  };

  return (
    <div className="rows-editor">
      {details.fileLinks.map((f) => (
        <div key={f.id} className="file-link">
          <div className="editor-row">
            <span className="file-kind">{t.file.kinds[f.kind]}</span>
            <AutoInput
              className="small"
              value={f.title}
              placeholder={t.file.titlePlaceholder}
              onCommit={(n) => run(() => api.updateFileLink(f.id, f.kind, f.target, n))}
            />
            <AutoInput
              className="grow"
              value={f.target}
              required
              title={f.target}
              onCommit={(v) => run(() => api.updateFileLink(f.id, f.kind, v, f.title))}
            />
            <button
              type="button"
              title={t.card.open}
              onClick={() =>
                api.openFileLink(f.kind, f.target).catch((e) => {
                  toasts.error(e);
                  void api.fileExists(f.target).then((ok) => !ok && setMissing((s) => new Set(s).add(f.id)));
                })
              }
            >
              ↗
            </button>
            <button type="button" title={t.card.remove} onClick={() => void remove(f)}>
              ×
            </button>
          </div>
          {missing.has(f.id) && (
            <div className="banner warn small-banner">
              {t.file.missing}{" "}
              <button type="button" className="link" onClick={() => void relocate(f)}>
                {t.file.relocate}
              </button>
            </div>
          )}
        </div>
      ))}
      <div className="editor-row add-row">
        <select value={kind} onChange={(e) => setKind(e.target.value as FileLinkKind)}>
          {KINDS.map((k) => (
            <option key={k} value={k}>
              {t.file.kinds[k]}
            </option>
          ))}
        </select>
        <input
          className="small"
          value={title}
          placeholder={t.file.titlePlaceholder}
          onChange={(e) => setTitle(e.target.value)}
        />
        <input
          className="grow"
          value={target}
          placeholder={kind === "local" ? t.file.pathPlaceholder : t.file.urlPlaceholder}
          onChange={(e) => setTarget(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        {kind === "local" && (
          <button
            type="button"
            onClick={async () => {
              const path = await api.pickFile().catch((e) => {
                toasts.error(e);
                return null;
              });
              if (path) setTarget(path);
            }}
          >
            {t.file.choose}
          </button>
        )}
        <button type="button" onClick={add} disabled={!target.trim()}>
          {t.card.add}
        </button>
      </div>
    </div>
  );
}
