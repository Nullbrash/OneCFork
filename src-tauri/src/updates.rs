//! Обновления: проверка и скачивание — в фоне, пока человек работает;
//! установка — при закрытии программы или по кнопке «Перезапустить сейчас»
//! (решение пользователя). Подлинность скачанного проверяет плагин по
//! подписи (публичный ключ — в tauri.conf.json).

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::backup::Trigger;
use crate::AppState;

/// Скачанное и проверенное обновление, ждущее установки.
pub struct PendingUpdate {
    pub update: Update,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub version: String,
    pub downloaded: u64,
    pub total: Option<u64>,
}

pub const EVENT_PROGRESS: &str = "update-progress";
pub const EVENT_READY: &str = "update-ready";
pub const EVENT_ERROR: &str = "update-error";

/// Не чаще — иначе на каждый кусок файла (килобайты) летело бы событие.
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// Проверить и скачать. Отладочная сборка себя не обновляет: она собрана
/// из исходников, а релиз на GitHub заменил бы её установленной версией.
pub async fn check_and_download(app: AppHandle) -> Result<Option<String>, String> {
    if cfg!(debug_assertions) {
        return Ok(None);
    }
    let updater = app.updater_builder().build().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let version = update.version.clone();
    let mut downloaded: u64 = 0;
    let mut last = Instant::now() - PROGRESS_EVERY;
    let emitter = app.clone();
    let progress_version = version.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                if last.elapsed() >= PROGRESS_EVERY || total.is_some_and(|t| downloaded >= t) {
                    last = Instant::now();
                    let _ = emitter.emit(
                        EVENT_PROGRESS,
                        UpdateProgress { version: progress_version.clone(), downloaded, total },
                    );
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    *state.pending_update.lock().map_err(|_| "update lock poisoned".to_string())? =
        Some(PendingUpdate { update, bytes });
    let _ = app.emit(EVENT_READY, version.clone());
    Ok(Some(version))
}

/// Поставить ждущее обновление. Установщик завершает процесс программы,
/// поэтому сначала — резервная копия (если были изменения).
/// `restart`: true — «Перезапустить сейчас», false — при закрытии.
pub fn install_pending(state: &AppState, restart: bool) -> Result<bool, String> {
    let pending = state.pending_update.lock().map_err(|_| "update lock poisoned".to_string())?.take();
    let Some(PendingUpdate { update, bytes }) = pending else {
        return Ok(false);
    };
    if let Err(e) = crate::commands::perform_backup(state, Trigger::Close) {
        eprintln!("backup before update failed: {e}");
    }
    update.restart_after_install(restart).install(bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Запуск проверки — один раз за работу программы (интерфейс вызывает после
/// входа; повторный вызов, например после перезагрузки окна, ничего не делает).
pub fn start_once(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.update_started.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = check_and_download(handle.clone()).await {
            let _ = handle.emit(EVENT_ERROR, e);
        }
    });
}
