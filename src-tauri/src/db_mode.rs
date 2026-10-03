//! Выбор рабочей или тестовой базы.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DbMode {
    Real,
    Test,
}

/// Флаг командной строки, включающий тестовую базу в любой сборке.
pub const TEST_DB_ARG: &str = "--test-db";
/// Переменная окружения: `test` или `real` — перекрывает выбор по типу сборки.
pub const DB_MODE_ENV: &str = "ONECFORK_DB";

impl DbMode {
    pub fn file_name(self) -> &'static str {
        match self {
            DbMode::Real => "onecfork.db",
            DbMode::Test => "onecfork-test.db",
        }
    }

    /// Отладочная сборка по умолчанию открывает ТЕСТОВУЮ базу: разработка и
    /// проверки не должны случайно попасть в рабочие данные. Рабочая база в
    /// отладке — только явным `ONECFORK_DB=real`.
    pub fn resolve(args: &[String], env_value: Option<&str>, debug_build: bool) -> DbMode {
        if args.iter().any(|a| a == TEST_DB_ARG) {
            return DbMode::Test;
        }
        match env_value.map(str::trim) {
            Some(v) if v.eq_ignore_ascii_case("test") => DbMode::Test,
            Some(v) if v.eq_ignore_ascii_case("real") => DbMode::Real,
            _ if debug_build => DbMode::Test,
            _ => DbMode::Real,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn debug_build_defaults_to_test() {
        assert_eq!(DbMode::resolve(&args(&[]), None, true), DbMode::Test);
    }

    #[test]
    fn release_build_defaults_to_real() {
        assert_eq!(DbMode::resolve(&args(&[]), None, false), DbMode::Real);
    }

    #[test]
    fn test_flag_wins_even_over_env_real() {
        assert_eq!(DbMode::resolve(&args(&["--test-db"]), Some("real"), false), DbMode::Test);
    }

    #[test]
    fn env_overrides_build_profile() {
        assert_eq!(DbMode::resolve(&args(&[]), Some("real"), true), DbMode::Real);
        assert_eq!(DbMode::resolve(&args(&[]), Some(" TEST "), false), DbMode::Test);
    }

    #[test]
    fn unknown_env_value_falls_back_to_profile() {
        assert_eq!(DbMode::resolve(&args(&[]), Some("prod"), true), DbMode::Test);
    }

    #[test]
    fn test_and_real_files_differ() {
        assert_ne!(DbMode::Real.file_name(), DbMode::Test.file_name());
    }
}
