//! Схема базы и её миграции. Номер версии — `PRAGMA user_version`.

use rusqlite::Connection;

/// Каждая миграция применяется один раз, по порядку; уже выпущенные — не
/// редактировать: у пользователя они уже применены, правка их не повторит.
const MIGRATIONS: &[&str] = &[SCHEMA_V1];

const SCHEMA_V1: &str = r#"
CREATE TABLE cards (
    id          INTEGER PRIMARY KEY,
    kind        TEXT    NOT NULL CHECK (kind IN ('company', 'person')),
    title       TEXT    NOT NULL CHECK (length(trim(title)) > 0),
    -- Ключ сортировки и сравнения: строчные буквы считаются в Rust, потому что
    -- COLLATE NOCASE в SQLite не понижает регистр кириллицы.
    title_key   TEXT    NOT NULL,
    note        TEXT    NOT NULL DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX cards_kind_title ON cards (kind, title_key);

-- Вид карточки не меняется: на нём держатся связи «компания — человек».
CREATE TRIGGER cards_kind_immutable BEFORE UPDATE OF kind ON cards
WHEN NEW.kind IS NOT OLD.kind
BEGIN
    SELECT RAISE(ABORT, 'cards.kind is immutable');
END;

CREATE TABLE company_people (
    company_id  INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    person_id   INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    position    TEXT    NOT NULL DEFAULT '',
    PRIMARY KEY (company_id, person_id)
);
CREATE INDEX company_people_person ON company_people (person_id);

CREATE TRIGGER company_people_kinds BEFORE INSERT ON company_people
BEGIN
    SELECT RAISE(ABORT, 'company_people.company_id must reference a company card')
    WHERE (SELECT kind FROM cards WHERE id = NEW.company_id) IS NOT 'company';
    SELECT RAISE(ABORT, 'company_people.person_id must reference a person card')
    WHERE (SELECT kind FROM cards WHERE id = NEW.person_id) IS NOT 'person';
END;

-- Три словаря одинакового устройства: заранее заданные значения + свои.
CREATE TABLE roles (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    name_key    TEXT    NOT NULL UNIQUE,
    is_preset   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE specializations (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    name_key    TEXT    NOT NULL UNIQUE,
    is_preset   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE address_labels (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    name_key    TEXT    NOT NULL UNIQUE,
    is_preset   INTEGER NOT NULL DEFAULT 0
);

INSERT INTO roles (name, name_key, is_preset) VALUES
    ('Поставщик', 'поставщик', 1),
    ('Монтажник', 'монтажник', 1),
    ('Доставщик', 'доставщик', 1),
    ('Заказчик',  'заказчик',  1);
INSERT INTO address_labels (name, name_key, is_preset) VALUES
    ('Офис',         'офис',         1),
    ('Склад',        'склад',        1),
    ('Производство', 'производство', 1);

CREATE TABLE card_roles (
    card_id     INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    role_id     INTEGER NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, role_id)
);
CREATE TABLE card_specializations (
    card_id           INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    specialization_id INTEGER NOT NULL REFERENCES specializations (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, specialization_id)
);

CREATE TABLE contacts (
    id          INTEGER PRIMARY KEY,
    card_id     INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    channel     TEXT    NOT NULL CHECK (channel IN
                    ('phone', 'telegram', 'whatsapp', 'viber', 'max', 'email', 'other')),
    value       TEXT    NOT NULL CHECK (length(trim(value)) > 0),
    label       TEXT    NOT NULL DEFAULT '',
    position    INTEGER NOT NULL
);
CREATE INDEX contacts_card ON contacts (card_id, position);

CREATE TABLE addresses (
    id          INTEGER PRIMARY KEY,
    card_id     INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    -- Удалили пометку — адрес остаётся, просто без пометки.
    label_id    INTEGER REFERENCES address_labels (id) ON DELETE SET NULL,
    text        TEXT    NOT NULL CHECK (length(trim(text)) > 0),
    position    INTEGER NOT NULL
);
CREATE INDEX addresses_card ON addresses (card_id, position);

CREATE TABLE file_links (
    id          INTEGER PRIMARY KEY,
    card_id     INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL CHECK (kind IN ('yandex_disk', 'local')),
    target      TEXT    NOT NULL CHECK (length(trim(target)) > 0),
    title       TEXT    NOT NULL DEFAULT '',
    position    INTEGER NOT NULL
);
CREATE INDEX file_links_card ON file_links (card_id, position);
"#;

#[derive(Debug)]
pub enum MigrationError {
    /// База создана более новой версией программы — открывать её старой нельзя,
    /// иначе старый код может испортить незнакомые ему данные.
    NewerThanApp { db: u32, app: u32 },
    Db(rusqlite::Error),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::NewerThanApp { db, app } => {
                write!(f, "database schema v{db} is newer than this app (v{app})")
            }
            MigrationError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(e: rusqlite::Error) -> Self {
        MigrationError::Db(e)
    }
}

pub fn latest_version() -> u32 {
    MIGRATIONS.len() as u32
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

/// Доводит схему до последней версии. Каждая миграция — в своей транзакции
/// вместе с записью номера версии: сбой посередине не оставит полусхему.
pub fn migrate(conn: &mut Connection) -> Result<u32, MigrationError> {
    let mut version = current_version(conn)?;
    let latest = latest_version();
    if version > latest {
        return Err(MigrationError::NewerThanApp { db: version, app: latest });
    }
    while version < latest {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATIONS[version as usize])?;
        version += 1;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(version)
}
