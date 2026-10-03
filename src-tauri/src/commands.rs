//! Команды, доступные интерфейсу. Тонкая прослойка: проверка и работа с
//! данными — в репозитории, сборка внешних ссылок — в `links`.

use std::path::Path;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::auth;
use crate::backup::{self, BackupCopy, BackupState, Trigger};
use crate::db_mode::DbMode;
use crate::export::{self, ExportField, KindLabels};
use crate::links::{self, MapService};
use crate::spreadsheet::{self, Sheet};
use crate::model::*;
use crate::repo::{RepoError, Repository, SqliteRepository};
use crate::AppState;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub mode: DbMode,
    pub db_path: String,
    pub schema_version: u32,
}

/// Ошибка для интерфейса: короткий код, по которому он выбирает текст из
/// словаря, и подробность для лога.
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub detail: String,
}

impl From<RepoError> for CommandError {
    fn from(e: RepoError) -> Self {
        let code = match e {
            RepoError::NotFound => "not_found",
            RepoError::EmptyValue(_) => "empty_value",
            RepoError::WrongKind(_) => "wrong_kind",
            RepoError::PresetTerm => "preset_term",
            RepoError::Migration(_) | RepoError::Db(_) | RepoError::Io(_) => "database",
        };
        CommandError { code, detail: e.to_string() }
    }
}

fn fail(code: &'static str, detail: impl ToString) -> CommandError {
    CommandError { code, detail: detail.to_string() }
}

type CmdResult<T> = Result<T, CommandError>;

/// Все команды с данными идут через эту функцию — и пока задан пароль, а
/// вход не выполнен, данные не отдаются (не только экран входа их прячет).
fn with_repo<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut SqliteRepository) -> Result<T, RepoError>,
) -> CmdResult<T> {
    if auth::is_set(&state.auth_file) && !state.unlocked.load(Ordering::SeqCst) {
        return Err(fail("locked", "password required"));
    }
    let mut repo = state.repo.lock().map_err(|_| fail("database", "repository lock poisoned"))?;
    Ok(f(&mut repo)?)
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    state.info.clone()
}

#[tauri::command]
pub fn list_rows(state: State<'_, AppState>, kind: Option<CardKind>) -> CmdResult<Vec<ListRow>> {
    with_repo(&state, |r| r.list_rows(kind))
}

#[tauri::command]
pub fn card_details(state: State<'_, AppState>, id: i64) -> CmdResult<CardDetails> {
    with_repo(&state, |r| r.card_details(id))
}

#[tauri::command]
pub fn create_card(state: State<'_, AppState>, kind: CardKind, title: String) -> CmdResult<i64> {
    with_repo(&state, |r| r.create_card(kind, &title, ""))
}

#[tauri::command]
pub fn update_card(state: State<'_, AppState>, id: i64, title: String, note: String) -> CmdResult<()> {
    with_repo(&state, |r| r.update_card(id, &title, &note))
}

#[tauri::command]
pub fn delete_card(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.soft_delete_card(id))
}

#[tauri::command]
pub fn restore_card(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.restore_card(id))
}

#[tauri::command]
pub fn purge_card(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.purge_card(id))
}

#[tauri::command]
pub fn purge_deleted(state: State<'_, AppState>) -> CmdResult<usize> {
    with_repo(&state, |r| r.purge_deleted())
}

#[tauri::command]
pub fn find_duplicates(
    state: State<'_, AppState>,
    title: String,
    phone: Option<String>,
    exclude: Option<i64>,
) -> CmdResult<Vec<Duplicate>> {
    with_repo(&state, |r| r.find_duplicates(&title, phone.as_deref(), exclude))
}

#[tauri::command]
pub fn list_terms(state: State<'_, AppState>, vocabulary: Vocabulary) -> CmdResult<Vec<Term>> {
    with_repo(&state, |r| r.list_terms(vocabulary))
}

#[tauri::command]
pub fn find_or_add_term(state: State<'_, AppState>, vocabulary: Vocabulary, name: String) -> CmdResult<i64> {
    with_repo(&state, |r| r.find_or_add_term(vocabulary, &name))
}

#[tauri::command]
pub fn delete_term(state: State<'_, AppState>, vocabulary: Vocabulary, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.delete_term(vocabulary, id))
}

#[tauri::command]
pub fn set_card_terms(
    state: State<'_, AppState>,
    card_id: i64,
    which: CardVocabulary,
    term_ids: Vec<i64>,
) -> CmdResult<()> {
    with_repo(&state, |r| r.set_card_terms(card_id, which, &term_ids))
}

#[tauri::command]
pub fn add_contact(
    state: State<'_, AppState>,
    card_id: i64,
    channel: Channel,
    value: String,
    label: String,
) -> CmdResult<i64> {
    with_repo(&state, |r| r.add_contact(card_id, channel, &value, &label))
}

#[tauri::command]
pub fn update_contact(
    state: State<'_, AppState>,
    id: i64,
    channel: Channel,
    value: String,
    label: String,
) -> CmdResult<()> {
    with_repo(&state, |r| r.update_contact(id, channel, &value, &label))
}

#[tauri::command]
pub fn remove_contact(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.remove_contact(id))
}

#[tauri::command]
pub fn add_address(
    state: State<'_, AppState>,
    card_id: i64,
    text: String,
    label_id: Option<i64>,
) -> CmdResult<i64> {
    with_repo(&state, |r| r.add_address(card_id, &text, label_id))
}

#[tauri::command]
pub fn update_address(state: State<'_, AppState>, id: i64, text: String, label_id: Option<i64>) -> CmdResult<()> {
    with_repo(&state, |r| r.update_address(id, &text, label_id))
}

#[tauri::command]
pub fn remove_address(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.remove_address(id))
}

#[tauri::command]
pub fn add_file_link(
    state: State<'_, AppState>,
    card_id: i64,
    kind: FileLinkKind,
    target: String,
    title: String,
) -> CmdResult<i64> {
    with_repo(&state, |r| r.add_file_link(card_id, kind, &target, &title))
}

#[tauri::command]
pub fn update_file_link(
    state: State<'_, AppState>,
    id: i64,
    kind: FileLinkKind,
    target: String,
    title: String,
) -> CmdResult<()> {
    with_repo(&state, |r| r.update_file_link(id, kind, &target, &title))
}

#[tauri::command]
pub fn remove_file_link(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.remove_file_link(id))
}

#[tauri::command]
pub fn link_person(
    state: State<'_, AppState>,
    company_id: i64,
    person_id: i64,
    position: String,
) -> CmdResult<()> {
    with_repo(&state, |r| r.link_person(company_id, person_id, &position))
}

#[tauri::command]
pub fn unlink_person(state: State<'_, AppState>, company_id: i64, person_id: i64) -> CmdResult<()> {
    with_repo(&state, |r| r.unlink_person(company_id, person_id))
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> CmdResult<Option<String>> {
    with_repo(&state, |r| r.get_setting(&key))
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    with_repo(&state, |r| r.set_setting(&key, &value))
}

#[tauri::command]
pub fn open_contact(app: AppHandle, channel: Channel, value: String) -> CmdResult<()> {
    let url = links::contact_url(channel, &value).ok_or_else(|| fail("no_link", "no chat link for value"))?;
    app.opener().open_url(url, None::<&str>).map_err(|e| fail("open_failed", e))
}

#[tauri::command]
pub fn open_map(app: AppHandle, service: MapService, address: String) -> CmdResult<()> {
    let url = links::map_url(service, &address).ok_or_else(|| fail("empty_value", "empty address"))?;
    app.opener().open_url(url, None::<&str>).map_err(|e| fail("open_failed", e))
}

#[tauri::command]
pub fn file_exists(path: String) -> bool {
    Path::new(path.trim()).exists()
}

#[tauri::command]
pub fn open_file_link(app: AppHandle, kind: FileLinkKind, target: String) -> CmdResult<()> {
    match kind {
        FileLinkKind::YandexDisk => {
            let url = links::web_link(&target).ok_or_else(|| fail("no_link", "not an http(s) link"))?;
            app.opener().open_url(url, None::<&str>).map_err(|e| fail("open_failed", e))
        }
        FileLinkKind::Local => {
            let path = target.trim();
            if !Path::new(path).exists() {
                return Err(fail("file_missing", path));
            }
            app.opener().open_path(path, None::<&str>).map_err(|e| fail("open_failed", e))
        }
    }
}

/// Async: системное окно выбора файла блокирует вызывающий поток, а
/// синхронные команды идут в главном потоке — окно программы бы зависло.
#[tauri::command]
pub async fn pick_file(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> CmdResult<()> {
    app.clipboard().write_text(text).map_err(|e| fail("copy_failed", e))
}

/// Async — по той же причине, что `pick_file`.
#[tauri::command]
pub async fn pick_spreadsheet(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("Excel / OpenDocument", &["xlsx", "xlsm", "xls", "ods"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

/// Async: большой файл читается заметное время — не держать главный поток.
#[tauri::command]
pub async fn read_spreadsheet(path: String) -> CmdResult<Vec<Sheet>> {
    spreadsheet::read_sheets(Path::new(path.trim())).map_err(|e| fail("spreadsheet", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateQuery {
    pub title: String,
    pub phone: Option<String>,
}

/// Дубли для всех строк импорта за один вызов (и одну блокировку базы).
#[tauri::command]
pub fn find_duplicates_batch(
    state: State<'_, AppState>,
    items: Vec<DuplicateQuery>,
) -> CmdResult<Vec<Vec<Duplicate>>> {
    with_repo(&state, |r| {
        items.iter().map(|q| r.find_duplicates(&q.title, q.phone.as_deref(), None)).collect()
    })
}

#[tauri::command]
pub fn import_rows(state: State<'_, AppState>, rows: Vec<ImportRow>) -> CmdResult<ImportReport> {
    with_repo(&state, |r| r.import_batch(&rows))
}

/// Async — по той же причине, что `pick_file`.
#[tauri::command]
pub async fn pick_export_template(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        // Только xlsx: записать с сохранением оформления умеем только его.
        .add_filter("Excel (.xlsx)", &["xlsx"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn pick_save_path(app: AppHandle, default_name: String) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("Excel (.xlsx)", &["xlsx"])
        .set_file_name(default_name)
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| {
            // Окно сохранения не дописывает расширение само, если его стёрли.
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xlsx")) {
                p
            } else {
                p.with_extension("xlsx")
            }
        })
        .map(|p| p.display().to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub template: String,
    pub sheet: String,
    pub header_row: u32,
    pub mapping: Vec<Option<ExportField>>,
    /// В порядке списка на экране.
    pub card_ids: Vec<i64>,
    pub kind_labels: KindLabels,
    pub output: String,
}

#[tauri::command]
pub fn export_cards(state: State<'_, AppState>, request: ExportRequest) -> CmdResult<usize> {
    let rows = with_repo(&state, |r| {
        let mut rows = Vec::with_capacity(request.card_ids.len());
        for id in &request.card_ids {
            match r.card_details(*id) {
                Ok(d) => rows.push(export::row_cells(&d, &request.mapping, &request.kind_labels)),
                // Карточку удалили, пока шёл экспорт, — просто без неё.
                Err(RepoError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(rows)
    })?;
    export::write_export(
        Path::new(request.template.trim()),
        &request.sheet,
        request.header_row,
        &request.mapping,
        &rows,
        Path::new(request.output.trim()),
    )
    .map_err(|e| match e {
        export::ExportError::SameAsTemplate => fail("same_as_template", e),
        _ => fail("export", e),
    })
}

#[tauri::command]
pub fn reveal_file(app: AppHandle, path: String) -> CmdResult<()> {
    app.opener().reveal_item_in_dir(path.trim()).map_err(|e| fail("open_failed", e))
}

// ---------- Пароль ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub password_set: bool,
    pub unlocked: bool,
}

fn auth_fail(e: auth::AuthError) -> CommandError {
    match e {
        auth::AuthError::WrongPassword => fail("wrong_password", e),
        auth::AuthError::Empty => fail("empty_value", e),
        _ => fail("auth", e),
    }
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> AuthStatus {
    let set = auth::is_set(&state.auth_file);
    AuthStatus { password_set: set, unlocked: !set || state.unlocked.load(Ordering::SeqCst) }
}

/// Async: Argon2 намеренно медленный — не держать главный поток окна.
#[tauri::command]
pub async fn unlock(state: State<'_, AppState>, password: String) -> CmdResult<bool> {
    let ok = auth::check(&state.auth_file, &password).map_err(auth_fail)?;
    if ok {
        state.unlocked.store(true, Ordering::SeqCst);
    }
    Ok(ok)
}

#[tauri::command]
pub async fn set_password(state: State<'_, AppState>, current: String, new: String) -> CmdResult<()> {
    auth::set(&state.auth_file, &current, &new).map_err(auth_fail)?;
    // Задавший пароль уже внутри программы — не запирать его сразу.
    state.unlocked.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn remove_password(state: State<'_, AppState>, current: String) -> CmdResult<()> {
    auth::remove(&state.auth_file, &current).map_err(auth_fail)
}

/// Скрытая кнопка сброса: показать файл пароля в проводнике — удалишь его,
/// и программа пустит без пароля.
#[tauri::command]
pub fn reveal_password_file(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let opener = app.opener();
    if state.auth_file.exists() {
        opener.reveal_item_in_dir(&state.auth_file).map_err(|e| fail("open_failed", e))
    } else {
        opener.open_path(state.data_dir.display().to_string(), None::<&str>).map_err(|e| fail("open_failed", e))
    }
}

// ---------- Резервные копии ----------

const BACKUP_DIR_KEY: &str = "backupDir";
const BACKUP_INTERVAL_KEY: &str = "backupIntervalMinutes";
const BACKUP_KEEP_KEY: &str = "backupKeep";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    /// Пусто — копии выключены (бэкап по желанию).
    pub dir: Option<String>,
    pub interval_minutes: u32,
    pub keep: u32,
}

fn read_backup_settings(repo: &SqliteRepository) -> Result<BackupSettings, RepoError> {
    let num = |key: &str, default: u32| -> Result<u32, RepoError> {
        Ok(repo.get_setting(key)?.and_then(|v| v.parse().ok()).unwrap_or(default))
    };
    Ok(BackupSettings {
        dir: repo.get_setting(BACKUP_DIR_KEY)?.filter(|d| !d.trim().is_empty()),
        interval_minutes: backup::clamp_interval(num(BACKUP_INTERVAL_KEY, backup::DEFAULT_INTERVAL_MINUTES)?),
        keep: backup::clamp_keep(num(BACKUP_KEEP_KEY, backup::DEFAULT_KEEP)?),
    })
}

/// Сделать копию, если это нужно по правилам `trigger`. `Ok(None)` — копия
/// не нужна или выключена. Вызывается и из команд, и из фонового потока, и
/// при закрытии окна — поэтому без `State`.
pub fn perform_backup(state: &AppState, trigger: Trigger) -> Result<Option<String>, String> {
    let _one_at_a_time = state.backup_lock.lock().map_err(|_| "backup lock poisoned".to_string())?;
    let repo = state.repo.lock().map_err(|_| "repository lock poisoned".to_string())?;
    let settings = read_backup_settings(&repo).map_err(|e| e.to_string())?;
    let Some(dir) = settings.dir else {
        return if trigger == Trigger::Manual { Err("no_backup_dir".into()) } else { Ok(None) };
    };
    let state_path = backup::state_file(&state.data_dir, state.mode);
    let mut bstate = backup::load_state(&state_path);
    let now = backup::now_secs();
    let db_modified = backup::modified_secs(&state.db_path);
    if !backup::should_backup(trigger, db_modified, &bstate, now, settings.interval_minutes) {
        return Ok(None);
    }
    let dir = Path::new(dir.trim());
    let target = dir.join(backup::backup_name(state.mode, now));
    let result = if dir.is_dir() {
        repo.backup_to(&target).map_err(|e| e.to_string())
    } else {
        Err("backup_dir_missing".to_string())
    };
    bstate.last_attempt_at = Some(now);
    match &result {
        Ok(()) => {
            backup::rotate(dir, state.mode, settings.keep);
            // Время файла базы на момент копии, а не «сейчас»: правка в ту же
            // секунду после копии не должна считаться уже скопированной.
            bstate.last_backup_at = Some(db_modified.unwrap_or(now));
            bstate.last_error = None;
        }
        Err(e) => bstate.last_error = Some(e.clone()),
    }
    backup::save_state(&state_path, &bstate);
    result.map(|_| Some(target.display().to_string()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub settings: BackupSettings,
    pub state: BackupState,
    pub copies: Vec<BackupCopy>,
}

#[tauri::command]
pub fn backup_status(state: State<'_, AppState>) -> CmdResult<BackupStatus> {
    let settings = with_repo(&state, |r| read_backup_settings(r))?;
    let copies = settings.dir.as_deref().map(|d| backup::list_copies(Path::new(d), state.mode)).unwrap_or_default();
    Ok(BackupStatus {
        state: backup::load_state(&backup::state_file(&state.data_dir, state.mode)),
        settings,
        copies,
    })
}

#[tauri::command]
pub fn set_backup_settings(
    state: State<'_, AppState>,
    dir: Option<String>,
    interval_minutes: u32,
    keep: u32,
) -> CmdResult<()> {
    with_repo(&state, |r| {
        r.set_setting(BACKUP_DIR_KEY, dir.as_deref().map(str::trim).unwrap_or(""))?;
        r.set_setting(BACKUP_INTERVAL_KEY, &backup::clamp_interval(interval_minutes).to_string())?;
        r.set_setting(BACKUP_KEEP_KEY, &backup::clamp_keep(keep).to_string())
    })
}

#[tauri::command]
pub async fn backup_now(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    // Та же проверка входа, что у остальных команд с данными.
    with_repo(&state, |_| Ok(()))?;
    perform_backup(&state, Trigger::Manual).map_err(|e| match e.as_str() {
        "no_backup_dir" => fail("no_backup_dir", e),
        "backup_dir_missing" => fail("backup_dir_missing", e),
        _ => fail("backup_failed", e),
    })
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

// ---------- Обновления ----------

#[tauri::command]
pub fn start_update_check(app: AppHandle) {
    crate::updates::start_once(&app);
}

/// «Перезапустить сейчас»: поставить скачанное обновление и открыть новую версию.
#[tauri::command]
pub fn install_update_now(state: State<'_, AppState>) -> CmdResult<bool> {
    crate::updates::install_pending(&state, true).map_err(|e| fail("update_failed", e))
}
