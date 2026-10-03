import { useEffect, useState } from "react";
import { api, type CardDetails, type ListRow, type Membership } from "../../api";
import { t } from "../../i18n";
import { AutoInput } from "../../ui/AutoInput";
import { Suggest } from "../../ui/Suggest";
import { useToasts } from "../../ui/toasts";
import type { Run } from "../CardScreen";

/** У компании — её люди; у человека — его компании. Связь «многие ко многим». */
export function MembershipsEditor({
  details,
  run,
  onOpen,
}: {
  details: CardDetails;
  run: Run;
  onOpen: (id: number) => void;
}) {
  const toasts = useToasts();
  const [candidates, setCandidates] = useState<ListRow[]>([]);
  const isCompany = details.card.kind === "company";
  const self = details.card.id;
  const otherKind = isCompany ? "person" : "company";

  useEffect(() => {
    api.listRows(otherKind).then(setCandidates).catch(toasts.error);
  }, [details, otherKind, toasts.error]);

  const pair = (otherId: number): [number, number] => (isCompany ? [self, otherId] : [otherId, self]);

  const link = (otherId: number, position: string) => {
    const [company, person] = pair(otherId);
    return api.linkPerson(company, person, position);
  };

  const unlink = (m: Membership) =>
    run(async () => {
      const [company, person] = pair(m.cardId);
      await api.unlinkPerson(company, person);
      toasts.show(t.toast.itemRemoved, { undo: () => void run(() => link(m.cardId, m.position)) });
    });

  const linked = details.memberships.map((m) => m.cardId);

  return (
    <div className="rows-editor">
      {details.memberships.map((m) => (
        <div key={m.cardId} className="editor-row">
          <button type="button" className="link grow left" onClick={() => onOpen(m.cardId)}>
            {m.title}
          </button>
          <AutoInput
            className="small"
            value={m.position}
            placeholder={t.membership.positionPlaceholder}
            onCommit={(p) => run(() => link(m.cardId, p))}
          />
          <button type="button" title={t.card.remove} onClick={() => void unlink(m)}>
            ×
          </button>
        </div>
      ))}
      <Suggest
        items={candidates}
        name={(c) => c.title}
        hint={(c) => [...c.roles, ...c.companies].join(", ")}
        exclude={(c) => linked.includes(c.id)}
        placeholder={isCompany ? t.membership.addPerson : t.membership.addCompany}
        onPick={(c) => void run(() => link(c.id, ""))}
        onCreate={(name) =>
          run(async () => {
            const id = await api.createCard(otherKind, name);
            await link(id, "");
          })
        }
      />
    </div>
  );
}
