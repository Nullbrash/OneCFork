//! Пароль на вход. Защита — от случайного человека за ПК, не от того, кто
//! откроет файл базы в обход программы (решение пользователя: без
//! шифрования). Поэтому пароль лежит отдельным файлом рядом с базой:
//! забыл пароль — удалил файл, данные целы.

use std::path::{Path, PathBuf};

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

use crate::db_mode::DbMode;

/// Свой файл у тестовой базы: проверки не должны трогать рабочий пароль.
pub fn auth_file(dir: &Path, mode: DbMode) -> PathBuf {
    dir.join(match mode {
        DbMode::Real => "onecfork-password.txt",
        DbMode::Test => "onecfork-test-password.txt",
    })
}

#[derive(Debug)]
pub enum AuthError {
    Empty,
    WrongPassword,
    Io(String),
    Hash(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Empty => write!(f, "password must not be empty"),
            AuthError::WrongPassword => write!(f, "wrong password"),
            AuthError::Io(e) => write!(f, "password file: {e}"),
            AuthError::Hash(e) => write!(f, "password hash: {e}"),
        }
    }
}

pub fn is_set(file: &Path) -> bool {
    file.exists()
}

/// Хранится только «отпечаток» (Argon2id со случайной солью), не сам пароль.
pub fn hash(password: &str) -> Result<String, AuthError> {
    if password.is_empty() {
        return Err(AuthError::Empty);
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AuthError::Hash(e.to_string()))
}

pub fn verify(password: &str, stored: &str) -> bool {
    match PasswordHash::new(stored.trim()) {
        Ok(parsed) => Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok(),
        // Испорченный файл не должен пускать с любым паролем.
        Err(_) => false,
    }
}

pub fn check(file: &Path, password: &str) -> Result<bool, AuthError> {
    if !is_set(file) {
        return Ok(true);
    }
    let stored = std::fs::read_to_string(file).map_err(|e| AuthError::Io(e.to_string()))?;
    Ok(verify(password, &stored))
}

/// Задать или сменить пароль; если он уже задан — нужен текущий.
pub fn set(file: &Path, current: &str, new: &str) -> Result<(), AuthError> {
    if is_set(file) && !check(file, current)? {
        return Err(AuthError::WrongPassword);
    }
    let h = hash(new)?;
    std::fs::write(file, h).map_err(|e| AuthError::Io(e.to_string()))
}

/// Убрать пароль (вход без пароля) — тоже только с текущим паролем.
pub fn remove(file: &Path, current: &str) -> Result<(), AuthError> {
    if !is_set(file) {
        return Ok(());
    }
    if !check(file, current)? {
        return Err(AuthError::WrongPassword);
    }
    std::fs::remove_file(file).map_err(|e| AuthError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_salted_and_verifies() {
        let a = hash("секрет 1").unwrap();
        let b = hash("секрет 1").unwrap();
        assert_ne!(a, b, "salt must differ");
        assert!(!a.contains("секрет"));
        assert!(verify("секрет 1", &a));
        assert!(!verify("секрет 2", &a));
        assert!(!verify("секрет 1", "испорчено"));
        assert!(matches!(hash(""), Err(AuthError::Empty)));
    }

    #[test]
    fn set_change_remove_require_current_password() {
        let dir = tempfile::tempdir().unwrap();
        let f = auth_file(dir.path(), DbMode::Test);
        assert!(!is_set(&f));
        assert!(check(&f, "что угодно").unwrap(), "no password — free entry");

        set(&f, "", "первый").unwrap();
        assert!(check(&f, "первый").unwrap());
        assert!(!check(&f, "другой").unwrap());

        assert!(matches!(set(&f, "неверный", "второй"), Err(AuthError::WrongPassword)));
        set(&f, "первый", "второй").unwrap();
        assert!(check(&f, "второй").unwrap());

        assert!(matches!(remove(&f, "первый"), Err(AuthError::WrongPassword)));
        remove(&f, "второй").unwrap();
        assert!(!is_set(&f));
    }

    #[test]
    fn deleting_the_file_resets_the_password() {
        let dir = tempfile::tempdir().unwrap();
        let f = auth_file(dir.path(), DbMode::Real);
        set(&f, "", "забытый").unwrap();
        std::fs::remove_file(&f).unwrap();
        assert!(check(&f, "").unwrap());
    }

    #[test]
    fn test_and_real_files_differ() {
        let d = Path::new("x");
        assert_ne!(auth_file(d, DbMode::Real), auth_file(d, DbMode::Test));
    }
}
