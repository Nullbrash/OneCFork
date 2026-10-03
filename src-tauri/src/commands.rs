//! Команды, доступные интерфейсу.

use serde::Serialize;
use tauri::State;

use crate::db_mode::DbMode;
use crate::AppState;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub mode: DbMode,
    pub db_path: String,
    pub schema_version: u32,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    state.info.clone()
}
