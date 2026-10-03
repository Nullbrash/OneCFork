//! Чтение таблиц Excel/ODS для импорта: всё превращается в текст, а
//! сопоставление столбцов с полями карточки делает интерфейс.

use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};
use serde::Serialize;

/// Пределы, чтобы случайно выбранный огромный файл не повесил программу.
pub const MAX_ROWS: usize = 10_000;
pub const MAX_COLS: usize = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sheet {
    pub name: String,
    /// Строки с начала листа (строка 1 Excel — индекс 0), без хвостовых пустых.
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum SpreadsheetError {
    Open(String),
    Read(String),
}

impl std::fmt::Display for SpreadsheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpreadsheetError::Open(e) => write!(f, "cannot open spreadsheet: {e}"),
            SpreadsheetError::Read(e) => write!(f, "cannot read sheet: {e}"),
        }
    }
}

/// Ячейка как текст. Телефоны в Excel часто хранятся числом: 79001234567
/// превратилось бы в «79001234567.0» или «7.9e10» — целые пишутся без дробной
/// части и экспоненты.
pub fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{f:.0}")
            } else {
                f.to_string()
            }
        }
        Data::Bool(b) => if *b { "да" } else { "нет" }.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::DateTime(d) => d.to_string(),
    }
}

pub fn read_sheets(path: &Path) -> Result<Vec<Sheet>, SpreadsheetError> {
    let mut book = open_workbook_auto(path).map_err(|e| SpreadsheetError::Open(e.to_string()))?;
    let mut sheets = Vec::new();
    for name in book.sheet_names() {
        let range = book.worksheet_range(&name).map_err(|e| SpreadsheetError::Read(e.to_string()))?;
        // Диапазон начинается с первой непустой ячейки — дополняем до A1, чтобы
        // номера строк совпадали с тем, что человек видит в Excel.
        let (row0, col0) = range.start().map(|(r, c)| (r as usize, c as usize)).unwrap_or((0, 0));
        let mut rows: Vec<Vec<String>> = vec![Vec::new(); row0.min(MAX_ROWS)];
        let mut truncated = false;
        for cells in range.rows() {
            if rows.len() >= MAX_ROWS {
                truncated = true;
                break;
            }
            let mut row: Vec<String> = vec![String::new(); col0.min(MAX_COLS)];
            row.extend(cells.iter().take(MAX_COLS.saturating_sub(col0)).map(cell_text));
            if cells.len() + col0 > MAX_COLS {
                truncated = true;
            }
            while row.last().is_some_and(String::is_empty) {
                row.pop();
            }
            rows.push(row);
        }
        while rows.last().is_some_and(Vec::is_empty) {
            rows.pop();
        }
        sheets.push(Sheet { name, rows, truncated });
    }
    Ok(sheets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::Workbook;

    #[test]
    fn numbers_become_clean_text() {
        assert_eq!(cell_text(&Data::Float(79001234567.0)), "79001234567");
        assert_eq!(cell_text(&Data::Float(2.5)), "2.5");
        assert_eq!(cell_text(&Data::Int(42)), "42");
        assert_eq!(cell_text(&Data::String("  Ателье  ".into())), "Ателье");
        assert_eq!(cell_text(&Data::Empty), "");
    }

    #[test]
    fn reads_xlsx_with_offset_and_numeric_phone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("contacts.xlsx");
        let mut book = Workbook::new();
        let sheet = book.add_worksheet().set_name("Контакты").unwrap();
        // Таблица начинается не с A1: B3 — заголовки.
        sheet.write_string(2, 1, "Название").unwrap();
        sheet.write_string(2, 2, "Телефон").unwrap();
        sheet.write_string(3, 1, "Ателье Тест").unwrap();
        sheet.write_number(3, 2, 79000001122.0).unwrap();
        book.add_worksheet().set_name("Пустой").unwrap();
        book.save(&path).unwrap();

        let sheets = read_sheets(&path).unwrap();
        assert_eq!(sheets.len(), 2);
        let s = &sheets[0];
        assert_eq!(s.name, "Контакты");
        assert_eq!(s.rows.len(), 4);
        assert!(s.rows[0].is_empty());
        assert_eq!(s.rows[2], ["", "Название", "Телефон"]);
        assert_eq!(s.rows[3], ["", "Ателье Тест", "79000001122"]);
        assert!(!s.truncated);
        assert!(sheets[1].rows.is_empty());
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(read_sheets(Path::new("Z:/nope/missing.xlsx")).is_err());
    }
}
