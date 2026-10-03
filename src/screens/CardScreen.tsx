import { useCallback, useEffect, useState } from "react";
import { api, type CardDetails, type CardKind, type Duplicate } from "../api";
import { t } from "../i18n";
import { AutoInput } from "../ui/AutoInput";
import { useToasts } from "../ui/toasts";
import { AddressesEditor } from "./editors/AddressesEditor";
import { ContactsEditor } from "./editors/ContactsEditor";
import { FileLinksEditor } from "./editors/FileLinksEditor";
import { MembershipsEditor } from "./editors/MembershipsEditor";
import { TermsEditor } from "./editors/TermsEditor";

interface Props {
  /** null — новая карточка, ещё не сохранённая (нет названия). */
  id: number | null;
  newKind: CardKind;
  onBack: () => void;
  onOpen: (id: number) => void;
  onCreated: (id: number) => void;
  onDeleted: (id: number, title: string) => void;
  /** Любая сохранённая правка — список должен перечитаться. */
  onChanged: () => void;
}

export function CardScreen({ id, newKind, onBack, onOpen, onCreated, onDeleted, onChanged }: Props) {
  const toasts = useToasts();
  const [details, setDetails] = useState<CardDetails | null>(null);
  const [duplicates, setDuplicates] = useState<Duplicate[]>([]);

  const reload = useCallback(async () => {
    if (id === null) return;
    try {
      setDetails(await api.cardDetails(id));
    } catch (e) {
      toasts.error(e);
      onBack();
    }
  }, [id, toasts, onBack]);

  // Другая карточка — другой экземпляр экрана (App задаёт key по id),
  // поэтому сбрасывать состояние здесь не нужно.
  useEffect(() => {
    if (id === null) return;
    let alive = true;
    api
      .cardDetails(id)
      .then((d) => alive && setDetails(d))
      .catch((e) => {
        toasts.error(e);
        onBack();
      });
    return () => {
      alive = false;
    };
  }, [id, toasts, onBack]);

  /** Сохранить правку → перечитать карточку и список. */
  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      try {
        await action();
        onChanged();
        await reload();
      } catch (e) {
        toasts.error(e);
        await reload();
      }
    },
    [onChanged, reload, toasts],
  );

  const checkDuplicates = useCallback(
    async (title: string, phone: string | null) => {
      try {
        const found = await api.findDuplicates(title, phone, id);
        setDuplicates((prev) => {
          const merged = [...prev];
          for (const d of found) if (!merged.some((m) => m.id === d.id)) merged.push(d);
          return merged;
        });
      } catch (e) {
        toasts.error(e);
      }
    },
    [id, toasts],
  );

  const kind = details?.card.kind ?? newKind;
  const titlePlaceholder = kind === "company" ? t.card.titleCompany : t.card.titlePerson;

  return (
    <div className="screen">
      <header className="toolbar">
        <button type="button" onClick={onBack}>
          {t.card.back}
        </button>
        <span className={`kind-badge ${kind}`}>{kind === "company" ? t.card.company : t.card.person}</span>
        {details && (
          <button
            type="button"
            className="danger push-right"
            onClick={async () => {
              try {
                await api.deleteCard(details.card.id);
                onDeleted(details.card.id, details.card.title);
              } catch (e) {
                toasts.error(e);
              }
            }}
          >
            {t.card.delete}
          </button>
        )}
      </header>

      <main className="card-body">
        {id === null ? (
          <>
            <AutoInput
              className="card-title"
              autoFocus
              placeholder={titlePlaceholder}
              value=""
              required
              onCommit={async (title) => {
                try {
                  onCreated(await api.createCard(newKind, title));
                } catch (e) {
                  toasts.error(e);
                }
              }}
            />
            <p className="muted">{t.card.draftHint}</p>
          </>
        ) : !details ? (
          <p className="muted">{t.loading}</p>
        ) : (
          <>
            <AutoInput
              className="card-title"
              placeholder={titlePlaceholder}
              value={details.card.title}
              required
              onCommit={(title) =>
                run(async () => {
                  await api.updateCard(details.card.id, title, details.card.note);
                  await checkDuplicates(title, null);
                })
              }
            />

            {duplicates.length > 0 && (
              <div className="banner warn">
                {t.card.duplicates}
                {duplicates.map((d) => (
                  <span key={d.id} className="dup">
                    <b>{d.title}</b> ({d.reason === "title" ? t.card.duplicateTitle : t.card.duplicatePhone}){" "}
                    <button type="button" className="link" onClick={() => onOpen(d.id)}>
                      {t.card.open}
                    </button>
                  </span>
                ))}
              </div>
            )}

            <TermsEditor details={details} run={run} />

            <section>
              <h3>{t.card.contacts}</h3>
              <ContactsEditor
                details={details}
                run={run}
                onPhone={(phone) => void checkDuplicates("", phone)}
              />
            </section>

            <section>
              <h3>{t.card.addresses}</h3>
              <AddressesEditor details={details} run={run} />
            </section>

            <section>
              <h3>{kind === "company" ? t.card.people : t.card.companies}</h3>
              <MembershipsEditor details={details} run={run} onOpen={onOpen} />
            </section>

            <section>
              <h3>{t.card.files}</h3>
              <FileLinksEditor details={details} run={run} />
            </section>

            <section>
              <h3>{t.card.note}</h3>
              <AutoInput
                multiline
                className="note"
                placeholder={t.card.notePlaceholder}
                value={details.card.note}
                onCommit={(note) => run(() => api.updateCard(details.card.id, details.card.title, note))}
              />
            </section>
          </>
        )}
      </main>
    </div>
  );
}

export type Run = (action: () => Promise<unknown>) => Promise<void>;
