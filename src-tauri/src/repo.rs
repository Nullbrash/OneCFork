//! Слой доступа к данным. Экраны и команды работают только через `Repository`,
//! чтобы SQLite можно было заменить серверной базой, не трогая их.

use std::collections::HashMap;
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
    Io(String),
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
            RepoError::Io(e) => write!(f, "file error: {e}"),
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
    /// Окончательное удаление (каскадом). Из интерфейса — только через
    /// `soft_delete_card` + окно отмены.
    fn delete_card(&mut self, id: i64) -> RepoResult<()>;
    fn soft_delete_card(&mut self, id: i64) -> RepoResult<()>;
    fn restore_card(&mut self, id: i64) -> RepoResult<()>;
    /// Стирает все помеченные удалёнными карточки; возвращает их число.
    fn purge_deleted(&mut self) -> RepoResult<usize>;
    /// Стирает одну помеченную удалённой карточку — по истечении её окна
    /// «Отменить»; окна других удалённых карточек не затрагиваются.
    fn purge_card(&mut self, id: i64) -> RepoResult<()>;
    fn get_card(&self, id: i64) -> RepoResult<Card>;
    fn list_cards(&self, kind: Option<CardKind>) -> RepoResult<Vec<Card>>;
    fn list_rows(&self, kind: Option<CardKind>) -> RepoResult<Vec<ListRow>>;
    fn card_details(&self, id: i64) -> RepoResult<CardDetails>;
    fn find_duplicates(&self, title: &str, phone: Option<&str>, exclude: Option<i64>)
        -> RepoResult<Vec<Duplicate>>;

    fn list_terms(&self, vocabulary: Vocabulary) -> RepoResult<Vec<Term>>;
    /// Найти значение по имени (без учёта регистра) или добавить новое.
    fn find_or_add_term(&mut self, vocabulary: Vocabulary, name: &str) -> RepoResult<i64>;
    fn delete_term(&mut self, vocabulary: Vocabulary, id: i64) -> RepoResult<()>;
    fn set_card_terms(&mut self, card_id: i64, which: CardVocabulary, term_ids: &[i64]) -> RepoResult<()>;

    fn add_contact(&mut self, card_id: i64, channel: Channel, value: &str, label: &str) -> RepoResult<i64>;
    fn update_contact(&mut self, id: i64, channel: Channel, value: &str, label: &str) -> RepoResult<()>;
    fn remove_contact(&mut self, id: i64) -> RepoResult<()>;
    fn add_address(&mut self, card_id: i64, text: &str, label_id: Option<i64>) -> RepoResult<i64>;
    fn update_address(&mut self, id: i64, text: &str, label_id: Option<i64>) -> RepoResult<()>;
    fn remove_address(&mut self, id: i64) -> RepoResult<()>;
    fn add_file_link(&mut self, card_id: i64, kind: FileLinkKind, target: &str, title: &str)
        -> RepoResult<i64>;
    fn update_file_link(&mut self, id: i64, kind: FileLinkKind, target: &str, title: &str) -> RepoResult<()>;
    fn remove_file_link(&mut self, id: i64) -> RepoResult<()>;

    /// Связать человека с компанией; повторный вызов меняет должность.
    fn link_person(&mut self, company_id: i64, person_id: i64, position: &str) -> RepoResult<()>;
    fn unlink_person(&mut self, company_id: i64, person_id: i64) -> RepoResult<()>;

    fn get_setting(&self, key: &str) -> RepoResult<Option<String>>;
    fn set_setting(&mut self, key: &str, value: &str) -> RepoResult<()>;

    /// Загрузка разобранных строк таблицы — целиком или никак: ошибка на
    /// любой строке откатывает весь импорт.
    fn import_batch(&mut self, rows: &[ImportRow]) -> RepoResult<ImportReport>;
}

/// Ключ сравнения: строчные буквы (включая кириллицу) и одиночные пробелы —
/// «Шторы», «шторы» и « шторы  » должны считаться одним значением.
pub fn text_key(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Ключ телефона: только цифры; российские 11-значные номера с 7 или 8 в
/// начале сводятся к 10 последним цифрам — «8 900 …» и «+7 900 …» совпадают.
pub fn phone_key(s: &str) -> String {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    if digits.len() == 11 && (digits.starts_with('7') || digits.starts_with('8')) {
        digits[1..].to_string()
    } else {
        digits
    }
}

fn contact_key(channel: Channel, value: &str) -> String {
    match channel {
        Channel::Phone | Channel::Whatsapp | Channel::Viber => phone_key(value),
        _ => text_key(value),
    }
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

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

pub struct SqliteRepository {
    conn: Connection,
}

impl SqliteRepository {
    pub fn open(path: &Path) -> RepoResult<Self> {
        Self::init(Connection::open(path)?)
    }

    /// Целостная копия базы в файл `target` (средствами SQLite: копия
    /// согласована, даже пока база открыта). Сначала пишется во временный
    /// файл и только потом переименовывается — оборванная копия не выдаст
    /// себя за готовую.
    pub fn backup_to(&self, target: &Path) -> RepoResult<()> {
        let partial = target.with_extension("partial");
        let _ = std::fs::remove_file(&partial);
        let result = self
            .conn
            .execute("VACUUM INTO ?1", [partial.display().to_string()])
            .map_err(RepoError::from)
            .and_then(|_| std::fs::rename(&partial, target).map_err(|e| RepoError::Io(e.to_string())));
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result
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

    /// Вид живой (не удалённой) карточки.
    fn card_kind(&self, id: i64) -> RepoResult<CardKind> {
        let kind: Option<String> = self
            .conn
            .query_row("SELECT kind FROM cards WHERE id = ?1 AND deleted_at IS NULL", [id], |r| r.get(0))
            .optional()?;
        let kind = kind.ok_or(RepoError::NotFound)?;
        Ok(CardKind::from_db(&kind).expect("cards.kind is constrained by CHECK"))
    }

    fn next_position(&self, table: &'static str, card_id: i64) -> RepoResult<i64> {
        let sql = format!("SELECT COALESCE(MAX(position), -1) + 1 FROM {table} WHERE card_id = ?1");
        Ok(self.conn.query_row(&sql, [card_id], |r| r.get(0))?)
    }

    fn touch(&self, card_id: i64) -> RepoResult<()> {
        self.conn.execute(&format!("UPDATE cards SET updated_at = {NOW} WHERE id = ?1"), [card_id])?;
        Ok(())
    }

    /// Обновляет `updated_at` карточки, которой принадлежит строка дочерней таблицы.
    fn touch_owner(&self, table: &'static str, row_id: i64) -> RepoResult<()> {
        let owner: Option<i64> = self
            .conn
            .query_row(&format!("SELECT card_id FROM {table} WHERE id = ?1"), [row_id], |r| r.get(0))
            .optional()?;
        match owner {
            Some(card_id) => self.touch(card_id),
            None => Err(RepoError::NotFound),
        }
    }

    /// Живая карточка вида `kind` с таким же названием (без учёта регистра).
    fn find_by_title(&self, kind: CardKind, title: &str) -> RepoResult<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM cards WHERE deleted_at IS NULL AND kind = ?1 AND title_key = ?2
                 ORDER BY id LIMIT 1",
                params![kind.as_db(), text_key(title)],
                |r| r.get(0),
            )
            .optional()?)
    }

    fn import_inner(&mut self, rows: &[ImportRow]) -> RepoResult<ImportReport> {
        let mut report = ImportReport::default();
        // Созданные этим импортом: повтор той же строки в файле (одна компания
        // в двух строках с разными телефонами) дополняет первую, а не плодит копию.
        let mut created: HashMap<(&'static str, String), i64> = HashMap::new();
        for row in rows {
            let title = row.title.trim();
            if row.action == ImportAction::Skip || title.is_empty() {
                report.skipped += 1;
                continue;
            }
            let key = (row.kind.as_db(), text_key(title));
            let (target, is_new) = match (row.action, created.get(&key)) {
                (ImportAction::Merge, _) => {
                    let id = row.merge_into.ok_or(RepoError::NotFound)?;
                    if self.card_kind(id)? != row.kind {
                        return Err(RepoError::WrongKind("merge target has another kind"));
                    }
                    report.merged += 1;
                    (id, false)
                }
                (_, Some(&id)) => {
                    report.merged += 1;
                    (id, false)
                }
                _ => {
                    let id = self.create_card(row.kind, title, &row.note)?;
                    created.insert(key, id);
                    report.created += 1;
                    (id, true)
                }
            };
            self.apply_import_row(target, row, is_new, &mut created, &mut report)?;
        }
        Ok(report)
    }

    /// Дописывает в карточку то, чего в ней ещё нет; существующее не трогает.
    fn apply_import_row(
        &mut self,
        id: i64,
        row: &ImportRow,
        is_new: bool,
        created: &mut HashMap<(&'static str, String), i64>,
        report: &mut ImportReport,
    ) -> RepoResult<()> {
        let note = row.note.trim();
        if !is_new && !note.is_empty() {
            let card = self.get_card(id)?;
            if !card.note.contains(note) {
                let merged =
                    if card.note.is_empty() { note.to_string() } else { format!("{}\n{note}", card.note) };
                self.update_card(id, &card.title, &merged)?;
            }
        }

        let details = self.card_details(id)?;
        let mut have: Vec<(Channel, String)> =
            details.contacts.iter().map(|c| (c.channel, contact_key(c.channel, &c.value))).collect();
        for c in &row.contacts {
            let value = c.value.trim();
            let k = (c.channel, contact_key(c.channel, value));
            if value.is_empty() || have.contains(&k) {
                continue;
            }
            self.add_contact(id, c.channel, value, "")?;
            have.push(k);
        }

        let mut addr: Vec<String> = details.addresses.iter().map(|a| text_key(&a.text)).collect();
        for a in &row.addresses {
            let k = text_key(a);
            if k.is_empty() || addr.contains(&k) {
                continue;
            }
            self.add_address(id, a, None)?;
            addr.push(k);
        }

        for (which, names, current) in [
            (CardVocabulary::Roles, &row.roles, &details.roles),
            (CardVocabulary::Specializations, &row.specializations, &details.specializations),
        ] {
            let mut ids: Vec<i64> = current.iter().map(|t| t.id).collect();
            let before = ids.len();
            for name in names.iter().filter(|n| !n.trim().is_empty()) {
                let term = self.find_or_add_term(which.vocabulary(), name)?;
                if !ids.contains(&term) {
                    ids.push(term);
                }
            }
            if ids.len() != before {
                self.set_card_terms(id, which, &ids)?;
            }
        }

        let company = row.company.trim();
        if row.kind == CardKind::Person && !company.is_empty() {
            let key = (CardKind::Company.as_db(), text_key(company));
            let company_id = match created.get(&key) {
                Some(&c) => c,
                None => match self.find_by_title(CardKind::Company, company)? {
                    Some(c) => c,
                    None => {
                        let c = self.create_card(CardKind::Company, company, "")?;
                        created.insert(key, c);
                        report.companies_created += 1;
                        c
                    }
                },
            };
            let existing = details.memberships.iter().find(|m| m.card_id == company_id);
            // Пустая должность из таблицы не затирает уже записанную.
            let position = match (row.position.trim(), existing) {
                ("", Some(m)) => m.position.clone(),
                (p, _) => p.to_string(),
            };
            self.link_person(company_id, id, &position)?;
        }
        Ok(())
    }

    fn card_terms(&self, card_id: i64, which: CardVocabulary) -> RepoResult<Vec<Term>> {
        let (link, column) = which.link();
        let table = which.vocabulary().table();
        let sql = format!(
            "SELECT t.id, t.name, t.is_preset FROM {table} t
             JOIN {link} l ON l.{column} = t.id
             WHERE l.card_id = ?1 ORDER BY t.is_preset DESC, t.name_key"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([card_id], |r| {
            Ok(Term { id: r.get(0)?, name: r.get(1)?, is_preset: r.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// card_id → (id, имя) значений словаря, для всех карточек разом.
    fn terms_by_card(&self, which: CardVocabulary) -> RepoResult<HashMap<i64, Vec<(i64, String)>>> {
        let (link, column) = which.link();
        let table = which.vocabulary().table();
        let sql = format!(
            "SELECT l.card_id, t.id, t.name FROM {link} l JOIN {table} t ON t.id = l.{column}
             ORDER BY l.card_id, t.is_preset DESC, t.name_key"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut map: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
        for row in stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
        })? {
            let (card, id, name) = row?;
            map.entry(card).or_default().push((id, name));
        }
        Ok(map)
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
            &format!(
                "UPDATE cards SET title = ?2, title_key = ?3, note = ?4, updated_at = {NOW}
                 WHERE id = ?1 AND deleted_at IS NULL"
            ),
            params![id, title, text_key(&title), note.trim()],
        )?;
        check_changed(rows)
    }

    fn delete_card(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute("DELETE FROM cards WHERE id = ?1", [id])?)
    }

    fn soft_delete_card(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute(
            &format!("UPDATE cards SET deleted_at = {NOW} WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
        )?)
    }

    fn restore_card(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute(
            "UPDATE cards SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
            [id],
        )?)
    }

    fn purge_deleted(&mut self) -> RepoResult<usize> {
        Ok(self.conn.execute("DELETE FROM cards WHERE deleted_at IS NOT NULL", [])?)
    }

    fn purge_card(&mut self, id: i64) -> RepoResult<()> {
        check_changed(self.conn.execute("DELETE FROM cards WHERE id = ?1 AND deleted_at IS NOT NULL", [id])?)
    }

    fn get_card(&self, id: i64) -> RepoResult<Card> {
        let sql = format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = ?1 AND deleted_at IS NULL");
        self.conn.query_row(&sql, [id], card_from_row).optional()?.ok_or(RepoError::NotFound)
    }

    fn list_cards(&self, kind: Option<CardKind>) -> RepoResult<Vec<Card>> {
        let sql = format!(
            "SELECT {CARD_COLUMNS} FROM cards
             WHERE deleted_at IS NULL AND (?1 IS NULL OR kind = ?1) ORDER BY title_key, id"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([kind.map(CardKind::as_db)], card_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn list_rows(&self, kind: Option<CardKind>) -> RepoResult<Vec<ListRow>> {
        let cards = self.list_cards(kind)?;
        let mut roles = self.terms_by_card(CardVocabulary::Roles)?;
        let mut specs = self.terms_by_card(CardVocabulary::Specializations)?;
        // Скрытый текст для поиска: собирается по карточкам из всех таблиц.
        let mut extra: HashMap<i64, Vec<String>> = HashMap::new();

        let mut companies: HashMap<i64, Vec<String>> = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT m.person_id, c.title, m.position FROM company_people m
             JOIN cards c ON c.id = m.company_id WHERE c.deleted_at IS NULL
             ORDER BY m.person_id, c.title_key",
        )?;
        for row in stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })? {
            let (person, title, position) = row?;
            companies.entry(person).or_default().push(title);
            extra.entry(person).or_default().push(position);
        }

        let mut phones: HashMap<i64, String> = HashMap::new();
        let mut phone_keys: HashMap<i64, Vec<String>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT card_id, channel, value, value_key, label FROM contacts ORDER BY card_id, position")?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })? {
            let (card, channel, value, key, label) = row?;
            if channel == "phone" {
                phones.entry(card).or_insert_with(|| value.clone());
            }
            if matches!(channel.as_str(), "phone" | "whatsapp" | "viber") && !key.is_empty() {
                phone_keys.entry(card).or_default().push(key);
            }
            let e = extra.entry(card).or_default();
            e.push(value);
            e.push(label);
        }

        let mut labels: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut stmt = self.conn.prepare("SELECT card_id, text, label_id FROM addresses ORDER BY card_id, position")?;
        for row in stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?))
        })? {
            let (card, text, label) = row?;
            extra.entry(card).or_default().push(text);
            if let Some(label) = label {
                let l = labels.entry(card).or_default();
                if !l.contains(&label) {
                    l.push(label);
                }
            }
        }

        let mut stmt = self.conn.prepare("SELECT card_id, title FROM file_links")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (card, title) = row?;
            extra.entry(card).or_default().push(title);
        }

        Ok(cards
            .into_iter()
            .map(|c| {
                let (role_ids, role_names): (Vec<i64>, Vec<String>) =
                    roles.remove(&c.id).unwrap_or_default().into_iter().unzip();
                let (spec_ids, spec_names): (Vec<i64>, Vec<String>) =
                    specs.remove(&c.id).unwrap_or_default().into_iter().unzip();
                let mut text = extra.remove(&c.id).unwrap_or_default();
                text.push(c.note);
                text.retain(|t| !t.trim().is_empty());
                ListRow {
                    roles: role_names,
                    specializations: spec_names,
                    companies: companies.remove(&c.id).unwrap_or_default(),
                    main_phone: phones.remove(&c.id),
                    role_ids,
                    specialization_ids: spec_ids,
                    address_label_ids: labels.remove(&c.id).unwrap_or_default(),
                    search_text: text.join("\n"),
                    phone_keys: phone_keys.remove(&c.id).unwrap_or_default(),
                    id: c.id,
                    kind: c.kind,
                    title: c.title,
                }
            })
            .collect())
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
                 JOIN cards c ON c.id = m.person_id
                 WHERE m.company_id = ?1 AND c.deleted_at IS NULL ORDER BY c.title_key"
            }
            CardKind::Person => {
                "SELECT c.id, c.title, m.position FROM company_people m
                 JOIN cards c ON c.id = m.company_id
                 WHERE m.person_id = ?1 AND c.deleted_at IS NULL ORDER BY c.title_key"
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

    fn find_duplicates(
        &self,
        title: &str,
        phone: Option<&str>,
        exclude: Option<i64>,
    ) -> RepoResult<Vec<Duplicate>> {
        let mut found: Vec<Duplicate> = Vec::new();
        let key = text_key(title);
        if !key.is_empty() {
            let mut stmt = self.conn.prepare(
                "SELECT id, kind, title FROM cards
                 WHERE deleted_at IS NULL AND title_key = ?1 AND (?2 IS NULL OR id <> ?2) ORDER BY id",
            )?;
            for row in stmt.query_map(params![key, exclude], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })? {
                let (id, kind, title) = row?;
                found.push(Duplicate {
                    id,
                    kind: CardKind::from_db(&kind).expect("cards.kind is constrained by CHECK"),
                    title,
                    reason: DuplicateReason::Title,
                });
            }
        }
        // Короткие обрывки номера совпадают с чем угодно — проверять только
        // номера, похожие на настоящие.
        let pkey = phone.map(phone_key).filter(|k| k.len() >= 6);
        if let Some(pkey) = pkey {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT c.id, c.kind, c.title FROM contacts k JOIN cards c ON c.id = k.card_id
                 WHERE c.deleted_at IS NULL AND k.channel IN ('phone', 'whatsapp', 'viber')
                   AND k.value_key = ?1 AND (?2 IS NULL OR c.id <> ?2) ORDER BY c.id",
            )?;
            for row in stmt.query_map(params![pkey, exclude], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })? {
                let (id, kind, title) = row?;
                if !found.iter().any(|d| d.id == id) {
                    found.push(Duplicate {
                        id,
                        kind: CardKind::from_db(&kind).expect("cards.kind is constrained by CHECK"),
                        title,
                        reason: DuplicateReason::Phone,
                    });
                }
            }
        }
        Ok(found)
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
        // Savepoint, а не transaction: метод вызывается и внутри импорта, где
        // уже открыта общая точка отката, а BEGIN внутри неё SQLite запрещает.
        let tx = self.conn.savepoint()?;
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
            "INSERT INTO contacts (card_id, channel, value, value_key, label, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![card_id, channel.as_db(), value, contact_key(channel, &value), label.trim(), position],
        )?;
        let id = self.conn.last_insert_rowid();
        self.touch(card_id)?;
        Ok(id)
    }

    fn update_contact(&mut self, id: i64, channel: Channel, value: &str, label: &str) -> RepoResult<()> {
        let value = required(value, "contact value")?;
        check_changed(self.conn.execute(
            "UPDATE contacts SET channel = ?2, value = ?3, value_key = ?4, label = ?5 WHERE id = ?1",
            params![id, channel.as_db(), value, contact_key(channel, &value), label.trim()],
        )?)?;
        self.touch_owner("contacts", id)
    }

    fn remove_contact(&mut self, id: i64) -> RepoResult<()> {
        self.touch_owner("contacts", id)?;
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

    fn update_address(&mut self, id: i64, text: &str, label_id: Option<i64>) -> RepoResult<()> {
        let text = required(text, "address")?;
        check_changed(self.conn.execute(
            "UPDATE addresses SET text = ?2, label_id = ?3 WHERE id = ?1",
            params![id, text, label_id],
        )?)?;
        self.touch_owner("addresses", id)
    }

    fn remove_address(&mut self, id: i64) -> RepoResult<()> {
        self.touch_owner("addresses", id)?;
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

    fn update_file_link(&mut self, id: i64, kind: FileLinkKind, target: &str, title: &str) -> RepoResult<()> {
        let target = required(target, "file link target")?;
        check_changed(self.conn.execute(
            "UPDATE file_links SET kind = ?2, target = ?3, title = ?4 WHERE id = ?1",
            params![id, kind.as_db(), target, title.trim()],
        )?)?;
        self.touch_owner("file_links", id)
    }

    fn remove_file_link(&mut self, id: i64) -> RepoResult<()> {
        self.touch_owner("file_links", id)?;
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

    fn import_batch(&mut self, rows: &[ImportRow]) -> RepoResult<ImportReport> {
        // SAVEPOINT, а не транзакция rusqlite: внутри используются обычные
        // методы репозитория, работающие с `self.conn`.
        self.conn.execute_batch("SAVEPOINT import_batch")?;
        match self.import_inner(rows) {
            Ok(report) => {
                self.conn.execute_batch("RELEASE import_batch")?;
                Ok(report)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK TO import_batch; RELEASE import_batch");
                Err(e)
            }
        }
    }

    fn get_setting(&self, key: &str) -> RepoResult<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    fn set_setting(&mut self, key: &str, value: &str) -> RepoResult<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
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
    fn v1_database_with_data_upgrades_to_latest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        {
            // База в том виде, в каком её оставила фаза 1 (только схема v1).
            let mut conn = Connection::open(&path).unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute_batch(crate::migrations::schema_v1_for_tests()).unwrap();
            tx.pragma_update(None, "user_version", 1).unwrap();
            tx.commit().unwrap();
            conn.execute(
                "INSERT INTO cards (kind, title, title_key) VALUES ('company', 'Старая', 'старая')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO contacts (card_id, channel, value, position) VALUES (1, 'phone', '+7 000', 0)",
                [],
            )
            .unwrap();
        }
        let r = SqliteRepository::open(&path).unwrap();
        assert_eq!(r.schema_version().unwrap(), migrations::latest_version());
        assert_eq!(r.list_rows(None).unwrap()[0].main_phone.as_deref(), Some("+7 000"));
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
    fn link_person_twice_updates_position() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Фирма", "").unwrap();
        let p = r.create_card(CardKind::Person, "Человек", "").unwrap();
        r.link_person(c, p, "Менеджер").unwrap();
        r.link_person(c, p, "Директор").unwrap();
        let m = r.card_details(c).unwrap().memberships;
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].position, "Директор");
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
    fn soft_delete_hides_card_and_restore_brings_back_everything() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Фирма", "").unwrap();
        let p = r.create_card(CardKind::Person, "Человек", "").unwrap();
        r.link_person(c, p, "Менеджер").unwrap();
        r.add_contact(c, Channel::Phone, "+7 000 111-22-33", "").unwrap();

        r.soft_delete_card(c).unwrap();
        assert!(matches!(r.get_card(c), Err(RepoError::NotFound)));
        assert_eq!(r.list_rows(None).unwrap().len(), 1);
        assert!(r.card_details(p).unwrap().memberships.is_empty());
        assert!(r.list_rows(Some(CardKind::Person)).unwrap()[0].companies.is_empty());
        assert!(r.find_duplicates("Фирма", None, None).unwrap().is_empty());
        assert!(matches!(r.add_contact(c, Channel::Phone, "1", ""), Err(RepoError::NotFound)));

        r.restore_card(c).unwrap();
        let d = r.card_details(c).unwrap();
        assert_eq!(d.contacts.len(), 1);
        assert_eq!(d.memberships[0].position, "Менеджер");
    }

    #[test]
    fn purge_removes_only_deleted_cards() {
        let mut r = repo();
        let keep = r.create_card(CardKind::Company, "Живая", "").unwrap();
        let gone = r.create_card(CardKind::Company, "Удалённая", "").unwrap();
        r.soft_delete_card(gone).unwrap();
        assert_eq!(r.purge_deleted().unwrap(), 1);
        assert!(r.get_card(keep).is_ok());
        assert!(matches!(r.restore_card(gone), Err(RepoError::NotFound)));
    }

    #[test]
    fn purge_card_touches_only_that_deleted_card() {
        let mut r = repo();
        let live = r.create_card(CardKind::Company, "Живая", "").unwrap();
        let a = r.create_card(CardKind::Company, "А", "").unwrap();
        let b = r.create_card(CardKind::Company, "Б", "").unwrap();
        r.soft_delete_card(a).unwrap();
        r.soft_delete_card(b).unwrap();
        r.purge_card(a).unwrap();
        r.restore_card(b).unwrap();
        assert!(matches!(r.purge_card(live), Err(RepoError::NotFound)));
        assert_eq!(r.list_cards(None).unwrap().len(), 2);
    }

    #[test]
    fn list_rows_shows_roles_specializations_companies_and_first_phone() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Ателье", "").unwrap();
        let p = r.create_card(CardKind::Person, "Мастер", "").unwrap();
        r.link_person(c, p, "").unwrap();
        let role = r.find_or_add_term(Vocabulary::Roles, "Монтажник").unwrap();
        let spec = r.find_or_add_term(Vocabulary::Specializations, "Шторы").unwrap();
        r.set_card_terms(p, CardVocabulary::Roles, &[role]).unwrap();
        r.set_card_terms(p, CardVocabulary::Specializations, &[spec]).unwrap();
        r.add_contact(p, Channel::Telegram, "@master", "").unwrap();
        r.add_contact(p, Channel::Phone, "+7 000 1", "").unwrap();
        r.add_contact(p, Channel::Phone, "+7 000 2", "").unwrap();

        let rows = r.list_rows(Some(CardKind::Person)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].roles, ["Монтажник"]);
        assert_eq!(rows[0].role_ids, [role]);
        assert_eq!(rows[0].specializations, ["Шторы"]);
        assert_eq!(rows[0].specialization_ids, [spec]);
        assert_eq!(rows[0].companies, ["Ателье"]);
        assert_eq!(rows[0].main_phone.as_deref(), Some("+7 000 1"));
    }

    #[test]
    fn list_rows_carries_hidden_search_text_phone_keys_and_label_ids() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Ателье", "звонить после 10").unwrap();
        let p = r.create_card(CardKind::Person, "Мастер", "").unwrap();
        r.link_person(c, p, "Бригадир").unwrap();
        r.add_contact(c, Channel::Telegram, "@atelier_test", "основной").unwrap();
        r.add_contact(c, Channel::Phone, "8 (900) 000-11-22", "").unwrap();
        let store = r.find_or_add_term(Vocabulary::AddressLabels, "Склад").unwrap();
        r.add_address(c, "ул. Складская, 5", Some(store)).unwrap();
        r.add_address(c, "ул. Вторая, 6", Some(store)).unwrap();
        r.add_file_link(c, FileLinkKind::Local, r"C:\x.xlsx", "Каталог тканей").unwrap();

        let rows = r.list_rows(None).unwrap();
        let company = rows.iter().find(|x| x.id == c).unwrap();
        for part in ["звонить после 10", "@atelier_test", "основной", "Складская", "Каталог тканей"] {
            assert!(company.search_text.contains(part), "missing {part}");
        }
        assert_eq!(company.phone_keys, ["9000001122"]);
        assert_eq!(company.address_label_ids, [store]);
        // Компания не ищется по своим людям — их имён в её тексте нет.
        assert!(!company.search_text.contains("Мастер"));
        let person = rows.iter().find(|x| x.id == p).unwrap();
        assert!(person.search_text.contains("Бригадир"));
    }

    #[test]
    fn duplicates_by_title_and_by_phone_in_any_format() {
        let mut r = repo();
        let a = r.create_card(CardKind::Company, "Ателье Тест", "").unwrap();
        let b = r.create_card(CardKind::Person, "Иван", "").unwrap();
        r.add_contact(b, Channel::Phone, "8 (900) 000-11-22", "").unwrap();

        let d = r.find_duplicates("  ателье   ТЕСТ ", None, None).unwrap();
        assert_eq!((d.len(), d[0].id, d[0].reason), (1, a, DuplicateReason::Title));
        assert!(r.find_duplicates("Ателье Тест", None, Some(a)).unwrap().is_empty());

        let d = r.find_duplicates("", Some("+7 900 000 11 22"), None).unwrap();
        assert_eq!((d.len(), d[0].id, d[0].reason), (1, b, DuplicateReason::Phone));
        assert!(r.find_duplicates("", Some("22"), None).unwrap().is_empty());
    }

    #[test]
    fn updates_of_contacts_addresses_and_links() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Фирма", "").unwrap();
        let k = r.add_contact(c, Channel::Phone, "1", "").unwrap();
        r.update_contact(k, Channel::Whatsapp, "+7 000 333-44-55", "рабочий").unwrap();
        let a = r.add_address(c, "Старый адрес", None).unwrap();
        let office = r.find_or_add_term(Vocabulary::AddressLabels, "Офис").unwrap();
        r.update_address(a, "Новый адрес", Some(office)).unwrap();
        let f = r.add_file_link(c, FileLinkKind::Local, "C:\\a.xlsx", "").unwrap();
        r.update_file_link(f, FileLinkKind::YandexDisk, "https://disk.yandex.ru/d/test", "Папка").unwrap();
        assert!(matches!(r.update_contact(k, Channel::Phone, "  ", ""), Err(RepoError::EmptyValue(_))));

        let d = r.card_details(c).unwrap();
        assert_eq!((d.contacts[0].channel, d.contacts[0].label.as_str()), (Channel::Whatsapp, "рабочий"));
        assert_eq!(d.addresses[0].label.as_ref().unwrap().name, "Офис");
        assert_eq!(d.file_links[0].kind, FileLinkKind::YandexDisk);
        // WhatsApp-номер тоже участвует в поиске дублей по телефону.
        assert_eq!(r.find_duplicates("", Some("89000"), None).unwrap().len(), 0);
        assert_eq!(r.find_duplicates("", Some("8 000 333 44 55"), None).unwrap().len(), 1);
    }

    #[test]
    fn settings_round_trip() {
        let mut r = repo();
        assert_eq!(r.get_setting("theme").unwrap(), None);
        r.set_setting("theme", "dark").unwrap();
        r.set_setting("theme", "light").unwrap();
        assert_eq!(r.get_setting("theme").unwrap().as_deref(), Some("light"));
    }

    #[test]
    fn operations_on_missing_card_report_not_found() {
        let mut r = repo();
        assert!(matches!(r.get_card(42), Err(RepoError::NotFound)));
        assert!(matches!(r.update_card(42, "x", ""), Err(RepoError::NotFound)));
        assert!(matches!(r.add_contact(42, Channel::Phone, "1", ""), Err(RepoError::NotFound)));
        assert!(matches!(r.remove_contact(42), Err(RepoError::NotFound)));
    }

    fn import_row(kind: CardKind, title: &str) -> ImportRow {
        ImportRow {
            kind,
            title: title.into(),
            contacts: vec![],
            addresses: vec![],
            roles: vec![],
            specializations: vec![],
            note: String::new(),
            company: String::new(),
            position: String::new(),
            action: ImportAction::Create,
            merge_into: None,
        }
    }

    fn phone(value: &str) -> ImportContact {
        ImportContact { channel: Channel::Phone, value: value.into() }
    }

    #[test]
    fn import_creates_cards_with_everything() {
        let mut r = repo();
        let mut row = import_row(CardKind::Company, "Ателье Тест");
        row.contacts = vec![phone("79000001122"), ImportContact { channel: Channel::Email, value: "a@example.com".into() }];
        row.addresses = vec!["ул. Примерная, 1".into()];
        row.roles = vec!["Поставщик".into(), " ".into()];
        row.specializations = vec!["Ткани".into()];
        row.note = "из таблицы".into();
        let rep = r.import_batch(&[row]).unwrap();
        assert_eq!(rep, ImportReport { created: 1, ..Default::default() });

        let d = r.card_details(r.list_cards(None).unwrap()[0].id).unwrap();
        assert_eq!(d.contacts.len(), 2);
        assert_eq!(d.addresses[0].text, "ул. Примерная, 1");
        assert_eq!(d.roles[0].name, "Поставщик"); // заранее заданная роль, не новая
        assert_eq!(r.list_terms(Vocabulary::Roles).unwrap().len(), 4);
        assert_eq!(d.specializations[0].name, "Ткани");
        assert_eq!(d.card.note, "из таблицы");
    }

    #[test]
    fn import_skips_rows_without_title_and_skip_action() {
        let mut r = repo();
        let mut skip = import_row(CardKind::Company, "Пропустить");
        skip.action = ImportAction::Skip;
        let rep = r.import_batch(&[import_row(CardKind::Company, "  "), skip]).unwrap();
        assert_eq!(rep.skipped, 2);
        assert!(r.list_cards(None).unwrap().is_empty());
    }

    #[test]
    fn repeated_rows_in_one_file_merge_into_the_first() {
        let mut r = repo();
        let mut a = import_row(CardKind::Company, "Фирма");
        a.contacts = vec![phone("8 900 000-11-22")];
        let mut b = import_row(CardKind::Company, " фирма ");
        b.contacts = vec![phone("+7 900 000 11 22"), phone("+7 900 000 33 44")];
        let rep = r.import_batch(&[a, b]).unwrap();
        assert_eq!((rep.created, rep.merged), (1, 1));
        let cards = r.list_cards(None).unwrap();
        assert_eq!(cards.len(), 1);
        // Тот же номер в другом формате не дублируется, новый — добавлен.
        assert_eq!(r.card_details(cards[0].id).unwrap().contacts.len(), 2);
    }

    #[test]
    fn merge_into_existing_adds_only_missing_data_and_appends_note() {
        let mut r = repo();
        let id = r.create_card(CardKind::Company, "Старая", "старая заметка").unwrap();
        r.add_contact(id, Channel::Phone, "8 900 000-11-22", "").unwrap();
        r.add_address(id, "ул. Первая, 1", None).unwrap();
        let mut row = import_row(CardKind::Company, "Старая из файла");
        row.action = ImportAction::Merge;
        row.merge_into = Some(id);
        row.contacts = vec![phone("79000001122"), phone("79000009999")];
        row.addresses = vec!["УЛ. ПЕРВАЯ, 1".into(), "ул. Вторая, 2".into()];
        row.note = "новая заметка".into();
        let rep = r.import_batch(&[row]).unwrap();
        assert_eq!(rep.merged, 1);
        let d = r.card_details(id).unwrap();
        assert_eq!(d.card.title, "Старая");
        assert_eq!(d.contacts.len(), 2);
        assert_eq!(d.addresses.len(), 2);
        assert_eq!(d.card.note, "старая заметка\nновая заметка");
    }

    #[test]
    fn people_are_linked_to_existing_or_new_companies() {
        let mut r = repo();
        let existing = r.create_card(CardKind::Company, "Ателье Тест", "").unwrap();
        let mut a = import_row(CardKind::Person, "Мария");
        a.company = "ателье тест".into();
        a.position = "Менеджер".into();
        let mut b = import_row(CardKind::Person, "Пётр");
        b.company = "Новая Фирма".into();
        let mut c = import_row(CardKind::Person, "Олег");
        c.company = "Новая фирма".into();
        let rep = r.import_batch(&[a, b, c]).unwrap();
        assert_eq!((rep.created, rep.companies_created), (3, 1));
        let ex = r.card_details(existing).unwrap();
        assert_eq!(ex.memberships[0].title, "Мария");
        assert_eq!(ex.memberships[0].position, "Менеджер");
        let new_firm = r.list_cards(Some(CardKind::Company)).unwrap().into_iter().find(|c| c.title == "Новая Фирма");
        assert_eq!(r.card_details(new_firm.unwrap().id).unwrap().memberships.len(), 2);
    }

    #[test]
    fn empty_position_does_not_overwrite_existing_one() {
        let mut r = repo();
        let c = r.create_card(CardKind::Company, "Фирма", "").unwrap();
        let p = r.create_card(CardKind::Person, "Иван", "").unwrap();
        r.link_person(c, p, "Бригадир").unwrap();
        let mut row = import_row(CardKind::Person, "Иван");
        row.action = ImportAction::Merge;
        row.merge_into = Some(p);
        row.company = "Фирма".into();
        r.import_batch(&[row]).unwrap();
        assert_eq!(r.card_details(c).unwrap().memberships[0].position, "Бригадир");
    }

    #[test]
    fn failing_row_rolls_back_the_whole_import() {
        let mut r = repo();
        let mut bad = import_row(CardKind::Company, "Плохая");
        bad.action = ImportAction::Merge;
        bad.merge_into = Some(999);
        let err = r.import_batch(&[import_row(CardKind::Company, "Хорошая"), bad]);
        assert!(matches!(err, Err(RepoError::NotFound)));
        assert!(r.list_cards(None).unwrap().is_empty());
        // После отката база в рабочем состоянии.
        r.create_card(CardKind::Company, "После", "").unwrap();
    }

    #[test]
    fn merge_into_card_of_another_kind_is_refused() {
        let mut r = repo();
        let person = r.create_card(CardKind::Person, "Иван", "").unwrap();
        let mut row = import_row(CardKind::Company, "Иван");
        row.action = ImportAction::Merge;
        row.merge_into = Some(person);
        assert!(matches!(r.import_batch(&[row]), Err(RepoError::WrongKind(_))));
    }

    #[test]
    fn backup_to_writes_a_complete_openable_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = SqliteRepository::open(&dir.path().join("live.db")).unwrap();
        r.create_card(CardKind::Company, "В копии", "").unwrap();
        let copy = dir.path().join("копия.db");
        r.backup_to(&copy).unwrap();
        assert!(!copy.with_extension("partial").exists());
        let restored = SqliteRepository::open(&copy).unwrap();
        assert_eq!(restored.list_cards(None).unwrap()[0].title, "В копии");
        // В несуществующую папку (вынутая флешка) — ошибка, без мусора.
        assert!(r.backup_to(&dir.path().join("нет/копия.db")).is_err());
    }

    #[test]
    fn keys_fold_case_spaces_and_phone_formats() {
        assert_eq!(text_key("  Шторы   и  КАРНИЗЫ "), "шторы и карнизы");
        assert_eq!(phone_key("8 (900) 123-45-67"), "9001234567");
        assert_eq!(phone_key("+7 900 123 45 67"), "9001234567");
        assert_eq!(phone_key("123-45"), "12345");
    }
}
