//! Сборка внешних ссылок из данных карточки. Открывается только то, что
//! собрано здесь: данные приходят из ввода и импорта, и произвольная строка
//! не должна превратиться в запуск чего угодно в системе.

use serde::Deserialize;

use crate::model::Channel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapService {
    Yandex,
    Google,
    Gis2,
}

/// Номер в международном виде без «+»: российские 10 и 11 (с 8) цифр → 7XXXXXXXXXX.
pub fn intl_digits(value: &str) -> Option<String> {
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    match digits.len() {
        10 => Some(format!("7{digits}")),
        11 if digits.starts_with('8') => Some(format!("7{}", &digits[1..])),
        n if (11..=15).contains(&n) => Some(digits),
        _ => None,
    }
}

fn is_http_url(value: &str) -> bool {
    let v = value.trim();
    (v.starts_with("https://") || v.starts_with("http://")) && !v.contains(char::is_whitespace)
}

/// Ссылка «открыть чат» для канала связи; `None` — только копирование
/// (телефон на ПК, MAX без известной ссылки на чат, непонятное значение).
pub fn contact_url(channel: Channel, value: &str) -> Option<String> {
    let v = value.trim();
    match channel {
        Channel::Phone | Channel::Max => None,
        Channel::Telegram => {
            if v.starts_with("https://t.me/") && !v.contains(char::is_whitespace) {
                return Some(v.to_string());
            }
            if v.starts_with('+') || v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return intl_digits(v).map(|d| format!("https://t.me/+{d}"));
            }
            let name = v.trim_start_matches('@');
            let valid = name.len() >= 4 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            valid.then(|| format!("https://t.me/{name}"))
        }
        Channel::Whatsapp => intl_digits(v).map(|d| format!("https://wa.me/{d}")),
        Channel::Viber => intl_digits(v).map(|d| format!("viber://chat?number=%2B{d}")),
        Channel::Email => {
            let ok = v.contains('@') && !v.contains(char::is_whitespace) && !v.contains(['?', '&', '<', '>']);
            ok.then(|| format!("mailto:{v}"))
        }
        Channel::Other => is_http_url(v).then(|| v.to_string()),
    }
}

/// Процентное кодирование для части URL: всё, кроме безопасных ASCII-символов.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn map_url(service: MapService, address: &str) -> Option<String> {
    let a = address.trim();
    if a.is_empty() {
        return None;
    }
    let q = encode_component(a);
    Some(match service {
        MapService::Yandex => format!("https://yandex.ru/maps/?text={q}"),
        MapService::Google => format!("https://www.google.com/maps/search/?api=1&query={q}"),
        MapService::Gis2 => format!("https://2gis.ru/search/{q}"),
    })
}

/// Ссылка на Яндекс-диск (или любой веб-адрес) — только http(s).
pub fn web_link(target: &str) -> Option<String> {
    is_http_url(target).then(|| target.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intl_digits_normalizes_russian_numbers() {
        assert_eq!(intl_digits("8 (900) 123-45-67").as_deref(), Some("79001234567"));
        assert_eq!(intl_digits("+7 900 123 45 67").as_deref(), Some("79001234567"));
        assert_eq!(intl_digits("900 123 45 67").as_deref(), Some("79001234567"));
        assert_eq!(intl_digits("+375 29 123 45 67").as_deref(), Some("375291234567"));
        assert_eq!(intl_digits("12-34"), None);
    }

    #[test]
    fn telegram_links() {
        assert_eq!(contact_url(Channel::Telegram, "@test_user").as_deref(), Some("https://t.me/test_user"));
        assert_eq!(contact_url(Channel::Telegram, "test_user").as_deref(), Some("https://t.me/test_user"));
        assert_eq!(
            contact_url(Channel::Telegram, "+7 900 123 45 67").as_deref(),
            Some("https://t.me/+79001234567")
        );
        assert_eq!(
            contact_url(Channel::Telegram, "https://t.me/test_user").as_deref(),
            Some("https://t.me/test_user")
        );
        assert_eq!(contact_url(Channel::Telegram, "@bad name"), None);
        assert_eq!(contact_url(Channel::Telegram, "javascript:alert(1)"), None);
    }

    #[test]
    fn whatsapp_viber_email_links() {
        assert_eq!(
            contact_url(Channel::Whatsapp, "8 900 123-45-67").as_deref(),
            Some("https://wa.me/79001234567")
        );
        assert_eq!(
            contact_url(Channel::Viber, "+7 900 123 45 67").as_deref(),
            Some("viber://chat?number=%2B79001234567")
        );
        assert_eq!(contact_url(Channel::Email, "test@example.com").as_deref(), Some("mailto:test@example.com"));
        assert_eq!(contact_url(Channel::Email, "test@example.com?subject=x"), None);
    }

    #[test]
    fn copy_only_channels_and_unsafe_values_give_no_link() {
        assert_eq!(contact_url(Channel::Phone, "+7 900 123 45 67"), None);
        assert_eq!(contact_url(Channel::Max, "+7 900 123 45 67"), None);
        assert_eq!(contact_url(Channel::Other, "file:///C:/Windows/system32/cmd.exe"), None);
        assert_eq!(contact_url(Channel::Other, "https://example.com/x").as_deref(), Some("https://example.com/x"));
    }

    #[test]
    fn map_urls_encode_address() {
        let a = "г. Тестовск, ул. Примерная, 1";
        let y = map_url(MapService::Yandex, a).unwrap();
        assert!(y.starts_with("https://yandex.ru/maps/?text="));
        assert!(!y.contains(' ') && !y.contains(','));
        assert!(map_url(MapService::Google, a).unwrap().starts_with("https://www.google.com/maps/search/"));
        assert!(map_url(MapService::Gis2, a).unwrap().starts_with("https://2gis.ru/search/"));
        assert_eq!(map_url(MapService::Yandex, "  "), None);
        assert_eq!(encode_component("a b&c"), "a%20b%26c");
    }

    #[test]
    fn web_link_only_http() {
        assert!(web_link("https://disk.yandex.ru/d/test").is_some());
        assert!(web_link("ftp://x").is_none());
        assert!(web_link("https://x y").is_none());
    }
}
