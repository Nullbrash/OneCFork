//! Пример таблицы для ручной проверки импорта. Данные выдуманные; файл
//! кладётся в `.incoming/` (вне git).
//!
//! cargo run --example make_sample_xlsx -- ../.incoming/sample_contacts.xlsx

use rust_xlsxwriter::{Format, Workbook, XlsxError};

fn main() -> Result<(), XlsxError> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "sample_contacts.xlsx".into());
    let bold = Format::new().set_bold();
    let mut book = Workbook::new();

    // Лист компаний: заголовок таблицы — не в первой строке, как часто бывает.
    let s = book.add_worksheet().set_name("Поставщики")?;
    s.write_string_with_format(0, 0, "Список поставщиков и подрядчиков (пример)", &bold)?;
    let headers = ["Название", "Телефон", "WhatsApp", "Адрес склада", "Услуги", "Примечание"];
    for (c, h) in headers.iter().enumerate() {
        s.write_string_with_format(2, c as u16, *h, &bold)?;
    }
    let rows: [[&str; 6]; 5] = [
        ["Ателье Пример", "8 900 000-11-22, 8 900 000-33-44", "", "г. Тестовск, ул. Примерная, 1", "Шторы, пошив", "звонить после 10"],
        ["Карнизы Тест", "", "+7 900 000 55 66", "г. Тестовск, пр. Выдуманный, 7", "Карнизы; монтаж", ""],
        ["", "", "", "", "", ""],
        ["ТК Доставка-Пример", "", "", "", "Доставка", "только по будням"],
        ["ателье пример", "8 900 000-77-88", "", "", "Ткани", "повтор строки — должен дополнить первую"],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            if !v.is_empty() {
                s.write_string(3 + r as u32, c as u16, *v)?;
            }
        }
    }
    // Телефон, записанный числом, — как его часто хранит Excel.
    s.write_number(6, 1, 79000009900.0)?;
    s.set_column_width(0, 22)?;
    s.set_column_width(1, 34)?;
    s.set_column_width(3, 34)?;

    // Лист людей: компания у людей — место работы.
    let p = book.add_worksheet().set_name("Люди")?;
    for (c, h) in ["ФИО", "Компания", "Должность", "Телефон", "Telegram"].iter().enumerate() {
        p.write_string_with_format(0, c as u16, *h, &bold)?;
    }
    let people: [[&str; 5]; 3] = [
        ["Мария Примерова", "Ателье Пример", "Менеджер", "8 900 000-12-12", "@maria_example"],
        ["Пётр Тестов", "Монтаж-Тест", "Бригадир", "8 900 000-13-13", ""],
        ["Олег Выдуманный", "", "Монтажник", "+7 900 000 14 14", ""],
    ];
    for (r, row) in people.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            if !v.is_empty() {
                p.write_string(1 + r as u32, c as u16, *v)?;
            }
        }
    }
    p.set_column_width(0, 22)?;
    p.set_column_width(1, 18)?;

    book.save(&path)?;
    println!("saved {path}");
    Ok(())
}
