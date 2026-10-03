//! Резервные копии базы в выбранную папку (флешка, другая папка).
//!
//! «Что-то изменилось» = файл базы записан позже последней копии. Учёт
//! копий хранится отдельным файлом, а не в базе: запись в базу о сделанной
//! копии сама сдвигала бы время файла — и копия делалась бы снова и снова.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::db_mode::DbMode;

pub const DEFAULT_INTERVAL_MINUTES: u32 = 60;
pub const DEFAULT_KEEP: u32 = 10;
const MIN_INTERVAL_MINUTES: u32 = 5;
const MAX_INTERVAL_MINUTES: u32 = 7 * 24 * 60;
const MAX_KEEP: u32 = 1000;

pub fn clamp_interval(minutes: u32) -> u32 {
    minutes.clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES)
}

pub fn clamp_keep(keep: u32) -> u32 {
    keep.clamp(1, MAX_KEEP)
}

/// Копии тестовой базы называются иначе — их нельзя спутать и смешать с
/// рабочими, и чистка одних не трогает другие.
pub fn prefix(mode: DbMode) -> &'static str {
    match mode {
        DbMode::Real => "onecfork-backup-",
        DbMode::Test => "onecfork-test-backup-",
    }
}

pub fn state_file(dir: &Path, mode: DbMode) -> PathBuf {
    dir.join(match mode {
        DbMode::Real => "onecfork-backup-state.json",
        DbMode::Test => "onecfork-test-backup-state.json",
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupState {
    /// Время (Unix, с) последней удачной копии.
    pub last_backup_at: Option<u64>,
    /// Последняя попытка — удачная или нет; от неё отсчитывается период.
    pub last_attempt_at: Option<u64>,
    pub last_error: Option<String>,
}

pub fn load_state(file: &Path) -> BackupState {
    std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_state(file: &Path, state: &BackupState) {
    if let Ok(json) = serde_json::to_string(state) {
        // Не удалось записать учёт — не повод ронять программу: в худшем
        // случае следующая копия сделается раньше.
        let _ = std::fs::write(file, json);
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn modified_secs(path: &Path) -> Option<u64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Раз в период, если что-то изменилось.
    Scheduled,
    /// При закрытии программы, если что-то изменилось.
    Close,
    /// Кнопка «Сделать копию сейчас» — всегда.
    Manual,
}

pub fn changed_since_backup(db_modified: Option<u64>, state: &BackupState) -> bool {
    match (db_modified, state.last_backup_at) {
        (_, None) => true,
        (Some(m), Some(b)) => m > b,
        // Время файла неизвестно — лучше лишняя копия, чем пропущенная.
        (None, Some(_)) => true,
    }
}

pub fn should_backup(
    trigger: Trigger,
    db_modified: Option<u64>,
    state: &BackupState,
    now: u64,
    interval_minutes: u32,
) -> bool {
    match trigger {
        Trigger::Manual => true,
        Trigger::Close => changed_since_backup(db_modified, state),
        Trigger::Scheduled => {
            let due = state
                .last_attempt_at
                .is_none_or(|t| now.saturating_sub(t) >= u64::from(interval_minutes) * 60);
            due && changed_since_backup(db_modified, state)
        }
    }
}

/// «2026-10-03_14-05-09» (UTC) — имена сортируются по времени как строки.
pub fn timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Перевод числа дней в дату (алгоритм Хиннанта), без зависимостей.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}_{:02}-{:02}-{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

pub fn backup_name(mode: DbMode, secs: u64) -> String {
    format!("{}{}.db", prefix(mode), timestamp(secs))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCopy {
    pub name: String,
    pub path: String,
    pub size: u64,
}

/// Только наши копии этого режима, новые — первыми.
pub fn list_copies(dir: &Path, mode: DbMode) -> Vec<BackupCopy> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let p = prefix(mode);
    let mut copies: Vec<BackupCopy> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            // Префиксы рабочих и тестовых копий не являются началом друг друга —
            // `starts_with` их не путает (проверено тестом ротации).
            if !name.starts_with(p) || !name.ends_with(".db") {
                return None;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            Some(BackupCopy { path: e.path().display().to_string(), name, size })
        })
        .collect();
    copies.sort_by(|a, b| b.name.cmp(&a.name));
    copies
}

/// Удаляет самые старые копии сверх `keep`; возвращает число удалённых.
pub fn rotate(dir: &Path, mode: DbMode, keep: u32) -> usize {
    list_copies(dir, mode)
        .into_iter()
        .skip(keep as usize)
        .filter(|c| std::fs::remove_file(&c.path).is_ok())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3600;

    #[test]
    fn scheduled_backup_needs_both_time_and_changes() {
        let state = BackupState { last_backup_at: Some(1000), last_attempt_at: Some(1000), last_error: None };
        // Час не прошёл — нет, даже если были изменения.
        assert!(!should_backup(Trigger::Scheduled, Some(2000), &state, 1000 + HOUR - 1, 60));
        // Час прошёл, но изменений не было — нет.
        assert!(!should_backup(Trigger::Scheduled, Some(900), &state, 1000 + HOUR, 60));
        // Час прошёл и были изменения — да.
        assert!(should_backup(Trigger::Scheduled, Some(2000), &state, 1000 + HOUR, 60));
    }

    #[test]
    fn first_backup_close_and_manual() {
        let empty = BackupState::default();
        assert!(should_backup(Trigger::Scheduled, Some(5), &empty, 10, 60));
        let state = BackupState { last_backup_at: Some(1000), last_attempt_at: Some(1000), last_error: None };
        assert!(should_backup(Trigger::Close, Some(1001), &state, 1001, 60));
        assert!(!should_backup(Trigger::Close, Some(1000), &state, 1001, 60));
        assert!(should_backup(Trigger::Manual, Some(1), &state, 1001, 60));
    }

    #[test]
    fn failed_attempt_waits_a_period_before_retry() {
        // Флешка вынута: попытка не удалась — не повторять каждую минуту.
        let state = BackupState { last_backup_at: None, last_attempt_at: Some(1000), last_error: Some("нет".into()) };
        assert!(!should_backup(Trigger::Scheduled, Some(1500), &state, 1000 + 60, 60));
        assert!(should_backup(Trigger::Scheduled, Some(1500), &state, 1000 + HOUR, 60));
    }

    #[test]
    fn timestamps_are_correct_and_sortable() {
        assert_eq!(timestamp(0), "1970-01-01_00-00-00");
        assert_eq!(timestamp(951_782_400), "2000-02-29_00-00-00"); // високосный день
        assert_eq!(timestamp(1_790_000_000), "2026-09-21_14-13-20");
        assert!(backup_name(DbMode::Real, 100) < backup_name(DbMode::Real, 200));
    }

    #[test]
    fn rotation_keeps_newest_and_ignores_foreign_files() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for i in 0..5u64 {
            std::fs::write(d.join(backup_name(DbMode::Real, 1_000 + i * HOUR)), b"x").unwrap();
        }
        std::fs::write(d.join(backup_name(DbMode::Test, 1_000)), b"t").unwrap();
        std::fs::write(d.join("мой документ.db"), b"keep").unwrap();
        std::fs::write(d.join("onecfork-backup-заметка.txt"), b"keep").unwrap();

        assert_eq!(rotate(d, DbMode::Real, 3), 2);
        let left = list_copies(d, DbMode::Real);
        assert_eq!(left.len(), 3);
        assert_eq!(left[0].name, backup_name(DbMode::Real, 1_000 + 4 * HOUR), "newest first");
        assert_eq!(list_copies(d, DbMode::Test).len(), 1, "test copies untouched");
        assert!(d.join("мой документ.db").exists());
        assert!(d.join("onecfork-backup-заметка.txt").exists());
    }

    #[test]
    fn state_round_trip_and_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let f = state_file(dir.path(), DbMode::Test);
        assert_eq!(load_state(&f), BackupState::default());
        let s = BackupState { last_backup_at: Some(1), last_attempt_at: Some(2), last_error: Some("e".into()) };
        save_state(&f, &s);
        assert_eq!(load_state(&f), s);
        std::fs::write(&f, "{мусор").unwrap();
        assert_eq!(load_state(&f), BackupState::default());
    }

    #[test]
    fn limits_are_clamped() {
        assert_eq!(clamp_interval(0), 5);
        assert_eq!(clamp_interval(60), 60);
        assert_eq!(clamp_keep(0), 1);
        assert_eq!(clamp_keep(5000), 1000);
    }
}
