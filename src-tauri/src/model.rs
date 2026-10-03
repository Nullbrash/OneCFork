//! Типы данных справочника.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CardKind {
    Company,
    Person,
}

impl CardKind {
    pub fn as_db(self) -> &'static str {
        match self {
            CardKind::Company => "company",
            CardKind::Person => "person",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "company" => Some(CardKind::Company),
            "person" => Some(CardKind::Person),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Phone,
    Telegram,
    Whatsapp,
    Viber,
    Max,
    Email,
    Other,
}

impl Channel {
    pub fn as_db(self) -> &'static str {
        match self {
            Channel::Phone => "phone",
            Channel::Telegram => "telegram",
            Channel::Whatsapp => "whatsapp",
            Channel::Viber => "viber",
            Channel::Max => "max",
            Channel::Email => "email",
            Channel::Other => "other",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "phone" => Channel::Phone,
            "telegram" => Channel::Telegram,
            "whatsapp" => Channel::Whatsapp,
            "viber" => Channel::Viber,
            "max" => Channel::Max,
            "email" => Channel::Email,
            "other" => Channel::Other,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileLinkKind {
    YandexDisk,
    Local,
}

impl FileLinkKind {
    pub fn as_db(self) -> &'static str {
        match self {
            FileLinkKind::YandexDisk => "yandex_disk",
            FileLinkKind::Local => "local",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "yandex_disk" => Some(FileLinkKind::YandexDisk),
            "local" => Some(FileLinkKind::Local),
            _ => None,
        }
    }
}

/// Словари «заранее заданные + свои значения».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vocabulary {
    Roles,
    Specializations,
    AddressLabels,
}

impl Vocabulary {
    /// Имя таблицы — только из этого перечня, никогда из ввода: в SQL его
    /// нельзя передать параметром, поэтому оно подставляется в текст запроса.
    pub(crate) fn table(self) -> &'static str {
        match self {
            Vocabulary::Roles => "roles",
            Vocabulary::Specializations => "specializations",
            Vocabulary::AddressLabels => "address_labels",
        }
    }
}

/// Словари, значения которых привязываются к карточке целиком.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardVocabulary {
    Roles,
    Specializations,
}

impl CardVocabulary {
    pub(crate) fn vocabulary(self) -> Vocabulary {
        match self {
            CardVocabulary::Roles => Vocabulary::Roles,
            CardVocabulary::Specializations => Vocabulary::Specializations,
        }
    }

    /// (таблица связи, столбец термина) — только из этого перечня.
    pub(crate) fn link(self) -> (&'static str, &'static str) {
        match self {
            CardVocabulary::Roles => ("card_roles", "role_id"),
            CardVocabulary::Specializations => ("card_specializations", "specialization_id"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i64,
    pub kind: CardKind,
    pub title: String,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Term {
    pub id: i64,
    pub name: String,
    pub is_preset: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: i64,
    pub channel: Channel,
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub id: i64,
    pub label: Option<Term>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLink {
    pub id: i64,
    pub kind: FileLinkKind,
    pub target: String,
    pub title: String,
}

/// Связь «компания — человек», видимая со стороны текущей карточки.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    /// Другая сторона связи: для компании — человек, для человека — компания.
    pub card_id: i64,
    pub title: String,
    pub position: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetails {
    pub card: Card,
    pub roles: Vec<Term>,
    pub specializations: Vec<Term>,
    pub contacts: Vec<Contact>,
    pub addresses: Vec<Address>,
    pub file_links: Vec<FileLink>,
    pub memberships: Vec<Membership>,
}
