//! Слой доступа к данным. Экраны и команды работают только через `Repository`,
//! чтобы SQLite можно было заменить серверной базой, не трогая их.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::migrations::{self, MigrationError};
use crate::model::*;

#[derive(Debug)]
pub enum RepoError {
    NotFound,
    EmptyValue(&'static str),
    WrongKind(&'static str),
    PresetTerm,
    Migration(MigrationError),
    Db(rusqlite::Error),
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoError::NotFound => write!(f, "not found"),
            RepoError::EmptyValue(what) => write!(f, "{what} must not be empty"),
            RepoError::WrongKind(what) => write!(f, "wrong card kind: {what}"),
            RepoError::PresetTerm => write!(f, "preset terms cannot be deleted"),
            RepoError::Migration(e) => write!(f, "{e}"),
            RepoError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for RepoError {}

impl From<rusqlite::Error> for RepoError {
    fn from(e: rusqlite::Error) -> Self {
        RepoError::Db(e)
    }
}

impl From<MigrationError> for RepoError {
    fn from(e: MigrationError) -> Self {
        RepoError::Migration(e)
    }
}

pub type RepoResult<T> = Result<T, RepoError>;

pub trait Repository {
    fn schema_version(&self) -> RepoResult<u32>;

    fn create_card(&mut self, kind: CardKind, title: &str, note: &str) -> RepoResult<i64>;
    fn update_card(&mut self, id: i64, title: &str, note: &str) -> RepoResult<()>;
    fn delete_card(&mut self, id: i64) -> RepoResult<()>;
    fn get_card(&self, id: i64) -> RepoResult<Card>;
    fn list_cards(&self, kind: Option<CardKind>) -> RepoResult<Vec<Card>>;
    fn card_details(&self, id: i64) -> RepoResult<CardDetails>;

    fn list_terms(&self, vocabulary: Vocabulary) -> RepoResult<Vec<Term>>;
    /// Найти значение по имени (без учёта регистра) или добавить новое.
    fn find_or_add_term(&mut self, vocabulary: Vocabulary, name: &str) -> RepoResult<i64>;
    fn delete_term(&mut self, vocabulary: Vocabulary, id: i64) -> RepoResult<()>;
    fn set_card_terms(&mut self, card_id: i64, which: CardVocabulary, term_ids: &[i64]) -> RepoResult<()>;

    fn add_contact(&mut self, card_id: i64, channel: Channel, value: &str, label: &str) -> RepoResult<i64>;
    fn remove_contact(&mut self, id: i64) -> RepoResult<()>;
    fn add_address(&mut self, card_id: i64, text: &str, label_id: Option<i64>) -> RepoResult<i64>;
    fn remove_address(&mut self, id: i64) -> RepoResult<()>;
    fn add_file_link(&mut self, card_id: i64, kind: FileLinkKind, target: &str, title: &str)
        -> RepoResult<i64>;
    fn remove_file_link(&mut self, id: i64) -> RepoResult<()>;

    fn link_person(&mut self, company_id: i64, person_id: i64, position: &str) -> RepoResult<()>;
    fn unlink_person(&mut self, company_id: i64, person_id: i64) -> RepoResult<()>;
}

/// Ключ сравнения: строчные буквы (включая кириллицу) и одиночные пробелы —
/// «Шторы», «шторы» и « шторы  » должны считаться одним значением.
pub fn text_key(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn required(value: &str, what: &'static str) -> RepoResult<String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(RepoError::EmptyValue(what));
    }
    Ok(v.to_string())
}

fn check_changed(rows: usize) -> RepoResult<()> {
    if rows == 0 {
        Err(RepoError::NotFound)
    } else {
        Ok(())
    }
}

pub struct SqliteRepository {
    conn: Connection,
}

impl SqliteRepository {
    pub fn open(path: &Path) -> RepoResult<Self> {
        Self::init(Connection::open(path)?)
    }

    /// Только для тестов: база в памяти, рабочие файлы не затрагиваются.
    pub fn open_in_memory() -> RepoResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> RepoResult<Self> {
        // Без этого SQLite молча игнорирует внешние ключи и каскадное удаление.
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrations::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    fn card_kind(&self, id: i64) -> RepoResult<CardKind> {
        let kind: Option<String> = self
            .conn
            .query_row("SELECT kind FROM cards WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        let kind = kind.ok_or(RepoError::NotFound)?;
        Ok(CardKind::from_db(&kind).expect("cards.kind is constrained by CHECK"))
    }

    fn next_position(&self, table: &'static str, card_id: i64) -> RepoResult<i64> {
        let sql = format!("SELECT COALESCE(MAX(position), -1) + 1 FROM {table} WHERE card_id = ?1");
        Ok(self.conn.query_row(&sql, [card_id], |r| r.get(0))?)
    }

    fn touch(&self, card_id: i64) -> RepoResult<()> {
        self.conn.execute(
            "UPDATE cards SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [card_id],
        )?;
        Ok(())
    }

    fn card_terms(&self, card_id: i64, which: CardVocabulary) -> RepoResult<Vec<Term>> {
        let (link, column) = which.link();
        let table = which.vocabulary().table();
        let sql = format!(
            "SELECT t.id, t.name, t.is_preset FROM {table} t
             JOIN {link} l ON l.{column} = t.id
             WHERE l.card_id = ?1 ORDER BY t.name_key"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([card_id], |r| {
            Ok(Term { id: r.get(0)?, name: r.get(1)?, is_preset: r.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn card_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
    let kind: String = r.get(1)?;
    Ok(Card {
        id: r.get(0)?,
        kind: CardKind::from_db(&kind).expect("cards.kind is constrained by CHECK"),
        title: r.get(2)?,
        note: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

const CARD_COLUMNS: &str = "id, kind, title, note, created_at, updated_at";

impl Repository for SqliteRepository {
    fn schema_version(&self) -> RepoResult<u32> {
        Ok(migrations::current_version(&self.conn)?)
    }

    fn create_card(&mut self, kind: CardKind, title: &str, note: &str) -> RepoResult<i64> {
        let title = required(title, "title")?;
        self.conn.execute(
            "INSERT INTO cards (kind, title, title_key, note) VALUES (?1, ?2, ?3, ?4)",
            params![kind.as_db(), title, text_key(&title), note.trim()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_card(&mut self, id: i64, title: &str, note: &str) -> RepoResult<()> {
        let title = required(title, "title")?;
        let rows = self.conn.execute(
            "UPDATE cards SET title = ?2, title_key = ?3, note = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id, title, text_key(&title), note.trim()],
        )?;
        check_changed(rows)
    }

    fn delete_card(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute("DELETE FROM cards WHERE id = ?1", [id])?)
    }

    fn get_card(&self, id: i64) -> RepoResult<Card> {
        let sql = format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = ?1");
        self.conn.query_row(&sql, [id], card_from_row).optional()?.ok_or(RepoError::NotFound)
    }

    fn list_cards(&self, kind: Option<CardKind>) -> RepoResult<Vec<Card>> {
        let sql = format!(
            "SELECT {CARD_COLUMNS} FROM cards WHERE ?1 IS NULL OR kind = ?1 ORDER BY title_key, id"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([kind.map(CardKind::as_db)], card_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn card_details(&self, id: i64) -> RepoResult<CardDetails> {
        let card = self.get_card(id)?;

        let mut stmt = self
            .conn
            .prepare("SELECT id, channel, value, label FROM contacts WHERE card_id = ?1 ORDER BY position")?;
        let contacts = stmt
            .query_map([id], |r| {
                let channel: String = r.get(1)?;
                Ok(Contact {
                    id: r.get(0)?,
                    channel: Channel::from_db(&channel).expect("contacts.channel is constrained by CHECK"),
                    value: r.get(2)?,
                    label: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.text, l.id, l.name, l.is_preset FROM addresses a
             LEFT JOIN address_labels l ON l.id = a.label_id
             WHERE a.card_id = ?1 ORDER BY a.position",
        )?;
        let addresses = stmt
            .query_map([id], |r| {
                let label_id: Option<i64> = r.get(2)?;
                let label = match label_id {
                    Some(label_id) => Some(Term { id: label_id, name: r.get(3)?, is_preset: r.get(4)? }),
                    None => None,
                };
                Ok(Address { id: r.get(0)?, label, text: r.get(1)? })
            })?
            .collect::<Result<_, _>>()?;

        let mut stmt = self
            .conn
            .prepare("SELECT id, kind, target, title FROM file_links WHERE card_id = ?1 ORDER BY position")?;
        let file_links = stmt
            .query_map([id], |r| {
                let kind: String = r.get(1)?;
                Ok(FileLink {
                    id: r.get(0)?,
                    kind: FileLinkKind::from_db(&kind).expect("file_links.kind is constrained by CHECK"),
                    target: r.get(2)?,
                    title: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        let sql = match card.kind {
            CardKind::Company => {
                "SELECT c.id, c.title, m.position FROM company_people m
                 JOIN cards c ON c.id = m.person_id WHERE m.company_id = ?1 ORDER BY c.title_key"
            }
            CardKind::Person => {
                "SELECT c.id, c.title, m.position FROM company_people m
                 JOIN cards c ON c.id = m.company_id WHERE m.person_id = ?1 ORDER BY c.title_key"
            }
        };
        let mut stmt = self.conn.prepare(sql)?;
        let memberships = stmt
            .query_map([id], |r| Ok(Membership { card_id: r.get(0)?, title: r.get(1)?, position: r.get(2)? }))?
            .collect::<Result<_, _>>()?;

        Ok(CardDetails {
            roles: self.card_terms(id, CardVocabulary::Roles)?,
            specializations: self.card_terms(id, CardVocabulary::Specializations)?,
            card,
            contacts,
            addresses,
            file_links,
            memberships,
        })
    }

    fn list_terms(&self, vocabulary: Vocabulary) -> RepoResult<Vec<Term>> {
        let sql = format!(
            "SELECT id, name, is_preset FROM {} ORDER BY is_preset DESC, name_key",
            vocabulary.table()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| Ok(Term { id: r.get(0)?, name: r.get(1)?, is_preset: r.get(2)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn find_or_add_term(&mut self, vocabulary: Vocabulary, name: &str) -> RepoResult<i64> {
        let name = required(name, "term name")?;
        let key = text_key(&name);
        let table = vocabulary.table();
        let existing: Option<i64> = self
            .conn
            .query_row(&format!("SELECT id FROM {table} WHERE name_key = ?1"), [&key], |r| r.get(0))
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        self.conn.execute(
            &format!("INSERT INTO {table} (name, name_key, is_preset) VALUES (?1, ?2, 0)"),
            params![name, key],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn delete_term(&mut self, vocabulary: Vocabulary, id: i64) -> RepoResult<()> {
        let table = vocabulary.table();
        let preset: Option<bool> = self
            .conn
            .query_row(&format!("SELECT is_preset FROM {table} WHERE id = ?1"), [id], |r| r.get(0))
            .optional()?;
        match preset {
            None => Err(RepoError::NotFound),
            Some(true) => Err(RepoError::PresetTerm),
            Some(false) => {
                self.conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
                Ok(())
            }
        }
    }

    fn set_card_terms(&mut self, card_id: i64, which: CardVocabulary, term_ids: &[i64]) -> RepoResult<()> {
        self.card_kind(card_id)?;
        let (link, column) = which.link();
        let tx = self.conn.transaction()?;
        tx.execute(&format!("DELETE FROM {link} WHERE card_id = ?1"), [card_id])?;
        for term_id in term_ids {
            tx.execute(
                &format!("INSERT OR IGNORE INTO {link} (card_id, {column}) VALUES (?1, ?2)"),
                params![card_id, term_id],
            )?;
        }
        tx.commit()?;
        self.touch(card_id)
    }

    fn add_contact(&mut self, card_id: i64, channel: Channel, value: &str, label: &str) -> RepoResult<i64> {
        self.card_kind(card_id)?;
        let value = required(value, "contact value")?;
        let position = self.next_position("contacts", card_id)?;
        self.conn.execute(
            "INSERT INTO contacts (card_id, channel, value, label, position) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![card_id, channel.as_db(), value, label.trim(), position],
        )?;
        let id = self.conn.last_insert_rowid();
        self.touch(card_id)?;
        Ok(id)
    }

    fn remove_contact(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute("DELETE FROM contacts WHERE id = ?1", [id])?)
    }

    fn add_address(&mut self, card_id: i64, text: &str, label_id: Option<i64>) -> RepoResult<i64> {
        self.card_kind(card_id)?;
        let text = required(text, "address")?;
        let position = self.next_position("addresses", card_id)?;
        self.conn.execute(
            "INSERT INTO addresses (card_id, label_id, text, position) VALUES (?1, ?2, ?3, ?4)",
            params![card_id, label_id, text, position],
        )?;
        let id = self.conn.last_insert_rowid();
        self.touch(card_id)?;
        Ok(id)
    }

    fn remove_address(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute("DELETE FROM addresses WHERE id = ?1", [id])?)
    }

    fn add_file_link(
        &mut self,
        card_id: i64,
        kind: FileLinkKind,
        target: &str,
        title: &str,
    ) -> RepoResult<i64> {
        self.card_kind(card_id)?;
        let target = required(target, "file link target")?;
        let position = self.next_position("file_links", card_id)?;
        self.conn.execute(
            "INSERT INTO file_links (card_id, kind, target, title, position) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![card_id, kind.as_db(), target, title.trim(), position],
        )?;
        let id = self.conn.last_insert_rowid();
        self.touch(card_id)?;
        Ok(id)
    }

    fn remove_file_link(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute("DELETE FROM file_links WHERE id = ?1", [id])?)
    }

    fn link_person(&mut self, company_id: i64, person_id: i64, position: &str) -> RepoResult<()> {
        if self.card_kind(company_id)? != CardKind::Company {
            return Err(RepoError::WrongKind("company_id must be a company"));
        }
        if self.card_kind(person_id)? != CardKind::Person {
            return Err(RepoError::WrongKind("person_id must be a person"));
        }
        self.conn.execute(
            "INSERT INTO company_people (company_id, person_id, position) VALUES (?1, ?2, ?3)
             ON CONFLICT (company_id, person_id) DO UPDATE SET position = excluded.position",
            params![company_id, person_id, position.trim()],
        )?;
        Ok(())
    }

    fn unlink_person(&mut self, company_id: i64, person_id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute(
            "DELETE FROM company_people WHERE company_id = ?1 AND person_id = ?2",
            params![company_id, person_id],
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> SqliteRepository {
        SqliteRepository::open_in_memory().expect("in-memory repo")
    }

    #[test]
    fn fresh_database_is_migrated_to_latest() {
        assert_eq!(repo().schema_version().unwrap(), migrations::latest_version());
    }

    #[test]
    fn reopening_file_database_keeps_data_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let id = {
            let mut r = SqliteRepository::open(&path).unwrap();
            r.create_card(CardKind::Company, "Ателье Тест", "").unwrap()
        };
        let r = SqliteRepository::open(&path).unwrap();
        assert_eq!(r.schema_version().unwrap(), migrations::latest_version());
        assert_eq!(r.get_card(id).unwrap().title, "Ателье Тест");
    }

    #[test]
    fn newer_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 999).unwrap();
        }
        assert!(matches!(
            SqliteRepository::open(&path),
            Err(RepoError::Migration(MigrationError::NewerThanApp { .. }))
        ));
    }

    #[test]
    fn presets_are_seeded() {
        let r = repo();
        let roles: Vec<String> = r.list_terms(Vocabulary::Roles).unwrap().into_iter().map(|t| t.name).collect();
        for name in ["Поставщик", "Монтажник", "Доставщик", "Заказчик"] {
            assert!(roles.contains(&name.to_string()), "missing preset role {name}");
        }
        assert_eq!(r.list_terms(Vocabulary::AddressLabels).unwrap().len(), 3);
        assert!(r.list_terms(Vocabulary::Specializations).unwrap().is_empty());
    }

    #[test]
    fn card_title_is_required_and_trimmed() {
        let mut r = repo();
        assert!(matches!(r.create_card(CardKind::Person, "   ", ""), Err(RepoError::EmptyValue(_))));
        let id = r.create_card(CardKind::Person, "  Иван Тестов ", " заметка ").unwrap();
        let card = r.get_card(id).unwrap();
        assert_eq!(card.title, "Иван Тестов");
        assert_eq!(card.note, "заметка");
    }

    #[test]
    fn list_cards_filters_by_kind_and_sorts_cyrillic_case_insensitively() {
        let mut r = repo();
        r.create_card(CardKind::Company, "яблоко", "").unwrap();
        r.create_card(CardKind::Company, "Арбуз", "").unwrap();
        r.create_card(CardKind::Person, "Борис", "").unwrap();
        let companies: Vec<String> =
            r.list_cards(Some(CardKind::Company)).unwrap().into_iter().map(|c| c.title).collect();
        assert_eq!(companies, ["Арбуз", "яблоко"]);
        assert_eq!(r.list_cards(None).unwrap().len(), 3);
    }

    #[test]
    fn custom_terms_are_deduplicated_ignoring_case_and_spaces() {
        let mut r = repo();
        let a = r.find_or_add_term(Vocabulary::Specializations, "Шторы").unwrap();
        let b = r.find_or_add_term(Vocabulary::Specializations, "  шторы ").unwrap();
        assert_eq!(a, b);
        let preset = r.find_or_add_term(Vocabulary::Roles, "монтажник").unwrap();
        assert!(r.list_terms(Vocabulary::Roles).unwrap().iter().any(|t| t.id == preset && t.is_preset));
    }

    #[test]
    fn preset_terms_cannot_be_deleted_custom_can() {
        let mut r = repo();
        let preset = r.list_terms(Vocabulary::Roles).unwrap()[0].id;
        assert!(matches!(r.delete_term(Vocabulary::Roles, preset), Err(RepoError::PresetTerm)));
        let custom = r.find_or_add_term(Vocabulary::Roles, "Замерщик").unwrap();
        r.delete_term(Vocabulary::Roles, custom).unwrap();
    }

    #[test]
    fn card_details_collects_everything() {
        let mut r = repo();
        let company = r.create_card(CardKind::Company, "Ателье Тест", "").unwrap();
        let person = r.create_card(CardKind::Person, "Мария Пример", "").unwrap();
        r.link_person(company, person, "Менеджер").unwrap();

        let role = r.find_or_add_term(Vocabulary::Roles, "Поставщик").unwrap();
        let spec = r.find_or_add_term(Vocabulary::Specializations, "Ткани").unwrap();
        r.set_card_terms(company, CardVocabulary::Roles, &[role]).unwrap();
        r.set_card_terms(company, CardVocabulary::Specializations, &[spec]).unwrap();

        r.add_contact(company, Channel::Phone, "+7 000 000-00-00", "").unwrap();
        r.add_contact(company, Channel::Telegram, "@test_atelier", "").unwrap();
        let warehouse = r.find_or_add_term(Vocabulary::AddressLabels, "склад").unwrap();
        r.add_address(company, "г. Тестовск, ул. Примерная, 1", Some(warehouse)).unwrap();
        r.add_file_link(company, FileLinkKind::Local, r"C:\Каталоги\ткани.xlsx", "Каталог").unwrap();

        let d = r.card_details(company).unwrap();
        assert_eq!(d.roles[0].name, "Поставщик");
        assert_eq!(d.specializations[0].name, "Ткани");
        assert_eq!(d.contacts.iter().map(|c| c.channel).collect::<Vec<_>>(), [Channel::Phone, Channel::Telegram]);
        assert_eq!(d.addresses[0].label.as_ref().unwrap().name, "Склад");
        assert_eq!(d.file_links[0].kind, FileLinkKind::Local);
        assert_eq!(d.memberships[0].title, "Мария Пример");
        assert_eq!(d.memberships[0].position, "Менеджер");

        let p = r.card_details(person).unwrap();
        assert_eq!(p.memberships[0].title, "Ателье Тест");
    }

    #[test]
    fn person_can_work_in_several_companies() {
        let mut r = repo();
        let a = r.create_card(CardKind::Company, "Фирма А", "").unwrap();
        let b = r.create_card(CardKind::Company, "Фирма Б", "").unwrap();
        let p = r.create_card(CardKind::Person, "Бригадир Тест", "").unwrap();
        r.link_person(a, p, "Бригадир").unwrap();
        r.link_person(b, p, "Бригадир").unwrap();
        assert_eq!(r.card_details(p).unwrap().memberships.len(), 2);
    }

    #[test]
    fn link_person_checks_kinds_in_repo_and_in_database() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Фирма", "").unwrap();
        let p = r.create_card(CardKind::Person, "Человек", "").unwrap();
        assert!(matches!(r.link_person(p, c, ""), Err(RepoError::WrongKind(_))));
        // Защита в самой базе — на случай будущего кода в обход репозитория.
        let raw = r.conn.execute("INSERT INTO company_people (company_id, person_id) VALUES (?1, ?2)", [p, c]);
        assert!(raw.is_err());
    }

    #[test]
    fn card_kind_cannot_change() {
        let r = repo();
        r.conn.execute("INSERT INTO cards (kind, title, title_key) VALUES ('company', 'Ф', 'ф')", []).unwrap();
        assert!(r.conn.execute("UPDATE cards SET kind = 'person'", []).is_err());
    }

    #[test]
    fn deleting_card_cascades_but_label_deletion_keeps_address() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Фирма", "").unwrap();
        let p = r.create_card(CardKind::Person, "Человек", "").unwrap();
        r.link_person(c, p, "").unwrap();
        r.add_contact(c, Channel::Email, "test@example.com", "").unwrap();
        let label = r.find_or_add_term(Vocabulary::AddressLabels, "Цех").unwrap();
        r.add_address(c, "Адрес", Some(label)).unwrap();

        r.delete_term(Vocabulary::AddressLabels, label).unwrap();
        let d = r.card_details(c).unwrap();
        assert_eq!(d.addresses.len(), 1);
        assert!(d.addresses[0].label.is_none());

        r.delete_card(c).unwrap();
        assert!(r.card_details(p).unwrap().memberships.is_empty());
        let left: i64 = r.conn.query_row("SELECT COUNT(*) FROM contacts", [], |x| x.get(0)).unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn operations_on_missing_card_report_not_found() {
        let mut r = repo();
        assert!(matches!(r.get_card(42), Err(RepoError::NotFound)));
        assert!(matches!(r.update_card(42, "x", ""), Err(RepoError::NotFound)));
        assert!(matches!(r.add_contact(42, Channel::Phone, "1", ""), Err(RepoError::NotFound)));
    }

    #[test]
    fn text_key_folds_case_and_spaces() {
        assert_eq!(text_key("  Шторы   и  КАРНИЗЫ "), "шторы и карнизы");
    }
}
