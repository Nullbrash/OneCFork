import { useState } from "react";
import { OPENABLE_CHANNELS, api, type CardDetails, type Channel, type Contact } from "../../api";
import { t } from "../../i18n";
import { AutoInput } from "../../ui/AutoInput";
import { useToasts } from "../../ui/toasts";
import type { Run } from "../CardScreen";

const CHANNELS: Channel[] = ["phone", "telegram", "whatsapp", "viber", "max", "email", "other"];
const PHONE_LIKE: Channel[] = ["phone", "whatsapp", "viber"];

export function ContactsEditor({
  details,
  run,
  onPhone,
}: {
  details: CardDetails;
  run: Run;
  onPhone: (phone: string) => void;
}) {
  const toasts = useToasts();
  const [channel, setChannel] = useState<Channel>("phone");
  const [value, setValue] = useState("");
  const [label, setLabel] = useState("");
  const cardId = details.card.id;

  const copy = (text: string) =>
    api
      .copyText(text)
      .then(() => toasts.show(t.toast.copied))
      .catch(toasts.error);

  const remove = (c: Contact) =>
    run(async () => {
      await api.removeContact(c.id);
      toasts.show(t.toast.itemRemoved, {
        undo: () => void run(() => api.addContact(cardId, c.channel, c.value, c.label)),
      });
    });

  const add = () => {
    if (!value.trim()) return;
    const [ch, v, l] = [channel, value, label];
    setValue("");
    setLabel("");
    void run(async () => {
      await api.addContact(cardId, ch, v, l);
      if (PHONE_LIKE.includes(ch)) onPhone(v);
    });
  };

  return (
    <div className="rows-editor">
      {details.contacts.map((c) => (
        <div key={c.id} className="editor-row">
          <select
            value={c.channel}
            onChange={(e) => run(() => api.updateContact(c.id, e.target.value as Channel, c.value, c.label))}
          >
            {CHANNELS.map((ch) => (
              <option key={ch} value={ch}>
                {t.contact.channels[ch]}
              </option>
            ))}
          </select>
          <AutoInput
            className="grow"
            value={c.value}
            required
            title={c.value}
            onContextMenu={(e) => {
              e.preventDefault();
              void copy(c.value);
            }}
            onCommit={(v) =>
              run(async () => {
                await api.updateContact(c.id, c.channel, v, c.label);
                if (PHONE_LIKE.includes(c.channel)) onPhone(v);
              })
            }
          />
          <AutoInput
            className="small"
            value={c.label}
            placeholder={t.contact.labelPlaceholder}
            onCommit={(l) => run(() => api.updateContact(c.id, c.channel, c.value, l))}
          />
          {OPENABLE_CHANNELS.includes(c.channel) && (
            <button
              type="button"
              title={t.contact.openChat}
              onClick={() => api.openContact(c.channel, c.value).catch(toasts.error)}
            >
              ↗
            </button>
          )}
          <button type="button" title={t.contact.copy} onClick={() => void copy(c.value)}>
            ⧉
          </button>
          <button type="button" title={t.card.remove} onClick={() => void remove(c)}>
            ×
          </button>
        </div>
      ))}
      <div className="editor-row add-row">
        <select value={channel} onChange={(e) => setChannel(e.target.value as Channel)}>
          {CHANNELS.map((ch) => (
            <option key={ch} value={ch}>
              {t.contact.channels[ch]}
            </option>
          ))}
        </select>
        <input
          className="grow"
          value={value}
          placeholder={t.contact.valuePlaceholder}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <input
          className="small"
          value={label}
          placeholder={t.contact.labelPlaceholder}
          onChange={(e) => setLabel(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <button type="button" onClick={add} disabled={!value.trim()}>
          {t.card.add}
        </button>
      </div>
    </div>
  );
}
