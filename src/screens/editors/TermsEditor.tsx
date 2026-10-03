import { useEffect, useState } from "react";
import { api, type CardDetails, type Term } from "../../api";
import { t } from "../../i18n";
import { Suggest } from "../../ui/Suggest";
import { useToasts } from "../../ui/toasts";
import type { Run } from "../CardScreen";

/** Роли — переключатели (их немного); специализации — выбор из своих + новые. */
export function TermsEditor({ details, run }: { details: CardDetails; run: Run }) {
  const toasts = useToasts();
  const [roles, setRoles] = useState<Term[]>([]);
  const [specs, setSpecs] = useState<Term[]>([]);
  const [newRole, setNewRole] = useState("");
  const cardId = details.card.id;

  useEffect(() => {
    Promise.all([api.listTerms("roles"), api.listTerms("specializations")])
      .then(([r, s]) => {
        setRoles(r);
        setSpecs(s);
      })
      .catch(toasts.error);
  }, [details, toasts.error]);

  const roleIds = details.roles.map((r) => r.id);
  const specIds = details.specializations.map((s) => s.id);

  const setRoleIds = (ids: number[]) => run(() => api.setCardTerms(cardId, "roles", ids));
  const setSpecIds = (ids: number[]) => run(() => api.setCardTerms(cardId, "specializations", ids));

  return (
    <>
      <section>
        <h3>{t.card.roles}</h3>
        <div className="chips">
          {roles.map((role) => {
            const on = roleIds.includes(role.id);
            return (
              <button
                key={role.id}
                type="button"
                className={on ? "chip on" : "chip"}
                onClick={() => setRoleIds(on ? roleIds.filter((x) => x !== role.id) : [...roleIds, role.id])}
              >
                {role.name}
              </button>
            );
          })}
          <input
            className="chip-input"
            value={newRole}
            placeholder={t.card.addRole}
            onChange={(e) => setNewRole(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== "Enter" || !newRole.trim()) return;
              e.stopPropagation();
              const name = newRole;
              setNewRole("");
              void run(async () => {
                const roleId = await api.findOrAddTerm("roles", name);
                if (!roleIds.includes(roleId)) await api.setCardTerms(cardId, "roles", [...roleIds, roleId]);
              });
            }}
          />
        </div>
      </section>

      <section>
        <h3>{t.card.specializations}</h3>
        <div className="chips">
          {details.specializations.map((spec) => (
            <span key={spec.id} className="chip on">
              {spec.name}
              <button
                type="button"
                className="chip-x"
                title={t.card.remove}
                onClick={() => setSpecIds(specIds.filter((x) => x !== spec.id))}
              >
                ×
              </button>
            </span>
          ))}
        </div>
        <Suggest
          items={specs}
          name={(s) => s.name}
          exclude={(s) => specIds.includes(s.id)}
          placeholder={t.card.addSpecialization}
          onPick={(s) => setSpecIds([...specIds, s.id])}
          onCreate={(name) =>
            run(async () => {
              const specId = await api.findOrAddTerm("specializations", name);
              if (!specIds.includes(specId))
                await api.setCardTerms(cardId, "specializations", [...specIds, specId]);
            })
          }
        />
      </section>
    </>
  );
}
