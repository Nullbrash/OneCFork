pub mod auth;
pub mod backup;
mod commands;
pub mod db_mode;
pub mod export;
pub mod links;
pub mod migrations;
pub mod model;
pub mod repo;
pub mod spreadsheet;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Duration;

use tauri::Manager;

use crate::backup::Trigger;
use crate::commands::AppInfo;
use crate::db_mode::{DbMode, DB_MODE_ENV};
use crate::repo::{Repository, SqliteRepository};

pub struct AppState {
    pub repo: Mutex<SqliteRepository>,
    pub info: AppInfo,
    pub mode: DbMode,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub auth_file: PathBuf,
    /// Вход выполнен в этом запуске (если пароль задан).
    pub unlocked: AtomicBool,
    /// Копии делаются по одной: фоновая, при закрытии и по кнопке не должны
    /// писать одновременно.
    pub backup_lock: Mutex<()>,
}

/// Как часто фоновый поток проверяет, не пора ли сделать копию. Сам период
/// копий — в настройках (по умолчанию час); минута — точность.
const BACKUP_CHECK_EVERY: Duration = Duration::from_secs(60);

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let env_mode = std::env::var(DB_MODE_ENV).ok();
    let mode = DbMode::resolve(&args, env_mode.as_deref(), cfg!(debug_assertions));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join(mode.file_name());
            let mut repo = SqliteRepository::open(&db_path)?;
            // Карточки, удалённые в прошлый раз, но не успевшие стереться
            // (программу закрыли в окне «Отменить»), — стираются сейчас.
            repo.purge_deleted()?;
            let info = AppInfo {
                version: app.package_info().version.to_string(),
                mode,
                db_path: db_path.display().to_string(),
                schema_version: repo.schema_version()?,
            };
            app.manage(AppState {
                repo: Mutex::new(repo),
                info,
                mode,
                auth_file: auth::auth_file(&dir, mode),
                data_dir: dir,
                db_path,
                unlocked: AtomicBool::new(false),
                backup_lock: Mutex::new(()),
            });

            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(BACKUP_CHECK_EVERY);
                let state = handle.state::<AppState>();
                if let Err(e) = commands::perform_backup(&state, Trigger::Scheduled) {
                    // Причина уже записана в учёт копий — её покажет экран настроек.
                    eprintln!("scheduled backup failed: {e}");
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                if let Err(e) = commands::perform_backup(&state, Trigger::Close) {
                    eprintln!("backup on close failed: {e}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_rows,
            commands::card_details,
            commands::create_card,
            commands::update_card,
            commands::delete_card,
            commands::restore_card,
            commands::purge_card,
            commands::purge_deleted,
            commands::find_duplicates,
            commands::list_terms,
            commands::find_or_add_term,
            commands::delete_term,
            commands::set_card_terms,
            commands::add_contact,
            commands::update_contact,
            commands::remove_contact,
            commands::add_address,
            commands::update_address,
            commands::remove_address,
            commands::add_file_link,
            commands::update_file_link,
            commands::remove_file_link,
            commands::link_person,
            commands::unlink_person,
            commands::get_setting,
            commands::set_setting,
            commands::open_contact,
            commands::open_map,
            commands::file_exists,
            commands::open_file_link,
            commands::pick_file,
            commands::copy_text,
            commands::pick_spreadsheet,
            commands::read_spreadsheet,
            commands::find_duplicates_batch,
            commands::import_rows,
            commands::pick_export_template,
            commands::pick_save_path,
            commands::export_cards,
            commands::reveal_file,
            commands::auth_status,
            commands::unlock,
            commands::set_password,
            commands::remove_password,
            commands::reveal_password_file,
            commands::backup_status,
            commands::set_backup_settings,
            commands::backup_now,
            commands::pick_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OneCFork");
}
