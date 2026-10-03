mod commands;
pub mod db_mode;
pub mod migrations;
pub mod model;
pub mod repo;

use std::sync::Mutex;

use tauri::Manager;

use crate::commands::AppInfo;
use crate::db_mode::{DbMode, DB_MODE_ENV};
use crate::repo::{Repository, SqliteRepository};

pub struct AppState {
    pub repo: Mutex<SqliteRepository>,
    pub info: AppInfo,
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let env_mode = std::env::var(DB_MODE_ENV).ok();
    let mode = DbMode::resolve(&args, env_mode.as_deref(), cfg!(debug_assertions));

    tauri::Builder::default()
        .setup(move |app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join(mode.file_name());
            let repo = SqliteRepository::open(&db_path)?;
            let info = AppInfo {
                version: app.package_info().version.to_string(),
                mode,
                db_path: db_path.display().to_string(),
                schema_version: repo.schema_version()?,
            };
            app.manage(AppState { repo: Mutex::new(repo), info });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("error while running OneCFork");
}
