import { useEffect, useState } from "react";
import { api, type Address, type CardDetails, type MapService, type Term } from "../../api";
import { t } from "../../i18n";
import { AutoInput } from "../../ui/AutoInput";
import { useToasts } from "../../ui/toasts";
import type { Run } from "../CardScreen";

const MAPS: MapService[] = ["yandex", "google", "gis2"];

export function AddressesEditor({ details, run }: { details: CardDetails; run: Run }) {
  const toasts = useToasts();
  const [labels, setLabels] = useState<Term[]>([]);
  const [text, setText] = useState("");
  const [labelId, setLabelId] = useState<number | null>(null);
  const [newLabel, setNewLabel] = useState("");
  const cardId = details.card.id;

  useEffect(() => {
    api.listTerms("addressLabels").then(setLabels).catch(toasts.error);
  }, [details, toasts.error]);

  const parseLabel = (raw: string): number | null => (raw === "" ? null : Number(raw));

  const addLabel = async () => {
    const name = newLabel.trim();
    if (!name) return;
    setNewLabel("");
    try {
      await api.findOrAddTerm("addressLabels", name);
      setLabels(await api.listTerms("addressLabels"));
    } catch (e) {
      toasts.error(e);
    }
  };

  const copy = (value: string) =>
    api
      .copyText(value)
      .then(() => toasts.show(t.toast.copied))
      .catch(toasts.error);

  const remove = (a: Address) =>
    run(async () => {
      await api.removeAddress(a.id);
      toasts.show(t.toast.itemRemoved, {
        undo: () => void run(() => api.addAddress(cardId, a.text, a.label?.id ?? null)),
      });
    });

  const add = () => {
    if (!text.trim()) return;
    const [v, l] = [text, labelId];
    setText("");
    void run(() => api.addAddress(cardId, v, l));
  };

  const labelSelect = (value: number | null, onChange: (raw: string) => void) => (
    <select value={value === null ? "" : String(value)} onChange={(e) => onChange(e.target.value)}>
      <option value="">{t.address.noLabel}</option>
      {labels.map((l) => (
        <option key={l.id} value={String(l.id)}>
          {l.name}
        </option>
      ))}
    </select>
  );

  return (
    <div className="rows-editor">
      {details.addresses.map((a) => (
        <div key={a.id} className="address">
          <div className="editor-row">
            {labelSelect(a.label?.id ?? null, (raw) =>
              run(() => api.updateAddress(a.id, a.text, parseLabel(raw))),
            )}
            <AutoInput
              className="grow"
              value={a.text}
              required
              title={t.address.copyHint}
              onContextMenu={(e) => {
                e.preventDefault();
                void copy(a.text);
              }}
              onCommit={(v) => run(() => api.updateAddress(a.id, v, a.label?.id ?? null))}
            />
            <button type="button" title={t.card.remove} onClick={() => void remove(a)}>
              ×
            </button>
          </div>
          <div className="maps">
            {MAPS.map((m) => (
              <button
                key={m}
                type="button"
                className="map-button"
                onClick={() => api.openMap(m, a.text).catch(toasts.error)}
              >
                {t.address.maps[m]}
              </button>
            ))}
          </div>
        </div>
      ))}
      <div className="editor-row add-row">
        {labelSelect(labelId, (raw) => setLabelId(parseLabel(raw)))}
        <input
          className="grow"
          value={text}
          placeholder={t.address.placeholder}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <button type="button" onClick={add} disabled={!text.trim()}>
          {t.card.add}
        </button>
      </div>
      <input
        className="chip-input"
        value={newLabel}
        placeholder={t.address.addLabel}
        onChange={(e) => setNewLabel(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && void addLabel()}
      />
    </div>
  );
}
