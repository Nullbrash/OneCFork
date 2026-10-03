import { invoke } from "@tauri-apps/api/core";

// Типы — зеркало src-tauri/src/model.rs (serde: camelCase / lowercase).

export type CardKind = "company" | "person";
export type Channel = "phone" | "telegram" | "whatsapp" | "viber" | "max" | "email" | "other";
export type FileLinkKind = "yandex_disk" | "local";
export type Vocabulary = "roles" | "specializations" | "addressLabels";
export type CardVocabulary = "roles" | "specializations";
export type MapService = "yandex" | "google" | "gis2";

export interface Card {
  id: number;
  kind: CardKind;
  title: string;
  note: string;
  createdAt: string;
  updatedAt: string;
}

export interface Term {
  id: number;
  name: string;
  isPreset: boolean;
}

export interface Contact {
  id: number;
  channel: Channel;
  value: string;
  label: string;
}

export interface Address {
  id: number;
  label: Term | null;
  text: string;
}

export interface FileLink {
  id: number;
  kind: FileLinkKind;
  target: string;
  title: string;
}

export interface Membership {
  cardId: number;
  title: string;
  position: string;
}

export interface CardDetails {
  card: Card;
  roles: Term[];
  specializations: Term[];
  contacts: Contact[];
  addresses: Address[];
  fileLinks: FileLink[];
  memberships: Membership[];
}

export interface ListRow {
  id: number;
  kind: CardKind;
  title: string;
  roles: string[];
  specializations: string[];
  companies: string[];
  mainPhone: string | null;
  roleIds: number[];
  specializationIds: number[];
  addressLabelIds: number[];
  /** Скрытый текст карточки для поиска (контакты, адреса, заметка…). */
  searchText: string;
  /** Ключи телефонов — цифры без кода страны. */
  phoneKeys: string[];
}

export interface Duplicate {
  id: number;
  kind: CardKind;
  title: string;
  reason: "title" | "phone";
}

export interface CommandError {
  code: string;
  detail: string;
}

export function errorCode(e: unknown): string {
  return typeof e === "object" && e !== null && "code" in e ? String((e as CommandError).code) : "unknown";
}

export const api = {
  listRows: (kind: CardKind | null) => invoke<ListRow[]>("list_rows", { kind }),
  cardDetails: (id: number) => invoke<CardDetails>("card_details", { id }),
  createCard: (kind: CardKind, title: string) => invoke<number>("create_card", { kind, title }),
  updateCard: (id: number, title: string, note: string) => invoke<void>("update_card", { id, title, note }),
  deleteCard: (id: number) => invoke<void>("delete_card", { id }),
  restoreCard: (id: number) => invoke<void>("restore_card", { id }),
  purgeCard: (id: number) => invoke<void>("purge_card", { id }),
  findDuplicates: (title: string, phone: string | null, exclude: number | null) =>
    invoke<Duplicate[]>("find_duplicates", { title, phone, exclude }),

  listTerms: (vocabulary: Vocabulary) => invoke<Term[]>("list_terms", { vocabulary }),
  findOrAddTerm: (vocabulary: Vocabulary, name: string) =>
    invoke<number>("find_or_add_term", { vocabulary, name }),
  setCardTerms: (cardId: number, which: CardVocabulary, termIds: number[]) =>
    invoke<void>("set_card_terms", { cardId, which, termIds }),

  addContact: (cardId: number, channel: Channel, value: string, label: string) =>
    invoke<number>("add_contact", { cardId, channel, value, label }),
  updateContact: (id: number, channel: Channel, value: string, label: string) =>
    invoke<void>("update_contact", { id, channel, value, label }),
  removeContact: (id: number) => invoke<void>("remove_contact", { id }),

  addAddress: (cardId: number, text: string, labelId: number | null) =>
    invoke<number>("add_address", { cardId, text, labelId }),
  updateAddress: (id: number, text: string, labelId: number | null) =>
    invoke<void>("update_address", { id, text, labelId }),
  removeAddress: (id: number) => invoke<void>("remove_address", { id }),

  addFileLink: (cardId: number, kind: FileLinkKind, target: string, title: string) =>
    invoke<number>("add_file_link", { cardId, kind, target, title }),
  updateFileLink: (id: number, kind: FileLinkKind, target: string, title: string) =>
    invoke<void>("update_file_link", { id, kind, target, title }),
  removeFileLink: (id: number) => invoke<void>("remove_file_link", { id }),

  linkPerson: (companyId: number, personId: number, position: string) =>
    invoke<void>("link_person", { companyId, personId, position }),
  unlinkPerson: (companyId: number, personId: number) =>
    invoke<void>("unlink_person", { companyId, personId }),

  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),

  openContact: (channel: Channel, value: string) => invoke<void>("open_contact", { channel, value }),
  openMap: (service: MapService, address: string) => invoke<void>("open_map", { service, address }),
  fileExists: (path: string) => invoke<boolean>("file_exists", { path }),
  openFileLink: (kind: FileLinkKind, target: string) => invoke<void>("open_file_link", { kind, target }),
  pickFile: () => invoke<string | null>("pick_file"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
};

/** Каналы, у которых на ПК есть «открыть чат»; остальные — только копирование. */
export const OPENABLE_CHANNELS: readonly Channel[] = ["telegram", "whatsapp", "viber", "email", "other"];
