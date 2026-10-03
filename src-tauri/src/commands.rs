//! Команды, доступные интерфейсу. Тонкая прослойка: проверка и работа с
//! данными — в репозитории, сборка внешних ссылок — в `links`.

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::db_mode::DbMode;
use crate::links::{self, MapService};
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
            RepoError::Migration(_) | RepoError::Db(_) => "database",
        };
        CommandError { code, detail: e.to_string() }
    }
}

fn fail(code: &'static str, detail: impl ToString) -> CommandError {
    CommandError { code, detail: detail.to_string() }
}

type CmdResult<T> = Result<T, CommandError>;

fn with_repo<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut SqliteRepository) -> Result<T, RepoError>,
) -> CmdResult<T> {
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
