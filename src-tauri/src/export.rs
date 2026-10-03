//! Экспорт карточек в Excel по образцу пользователя: его файл с
//! заголовками заполняется строками, оформление образца сохраняется.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::model::{CardDetails, CardKind, Channel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportField {
    Title,
    Kind,
    Phone,
    Telegram,
    Whatsapp,
    Viber,
    Max,
    Email,
    Address,
    Roles,
    Specializations,
    Note,
    Company,
    Position,
}

/// Подписи вида карточки — из словаря интерфейса (тексты живут там).
#[derive(Clone, Debug, Deserialize)]
pub struct KindLabels {
    pub company: String,
    pub person: String,
}

fn channel_of(field: ExportField) -> Option<Channel> {
    Some(match field {
        ExportField::Phone => Channel::Phone,
        ExportField::Telegram => Channel::Telegram,
        ExportField::Whatsapp => Channel::Whatsapp,
        ExportField::Viber => Channel::Viber,
        ExportField::Max => Channel::Max,
        ExportField::Email => Channel::Email,
        _ => return None,
    })
}

pub fn field_values(d: &CardDetails, field: ExportField, labels: &KindLabels) -> Vec<String> {
    if let Some(channel) = channel_of(field) {
        return d.contacts.iter().filter(|c| c.channel == channel).map(|c| c.value.clone()).collect();
    }
    match field {
        ExportField::Title => vec![d.card.title.clone()],
        ExportField::Kind => vec![match d.card.kind {
            CardKind::Company => labels.company.clone(),
            CardKind::Person => labels.person.clone(),
        }],
        ExportField::Address => d
            .addresses
            .iter()
            .map(|a| match &a.label {
                Some(l) => format!("{}: {}", l.name, a.text),
                None => a.text.clone(),
            })
            .collect(),
        ExportField::Roles => d.roles.iter().map(|t| t.name.clone()).collect(),
        ExportField::Specializations => d.specializations.iter().map(|t| t.name.clone()).collect(),
        ExportField::Note => vec![d.card.note.clone()],
        // У человека — его компании; у компании это поле пустое.
        ExportField::Company if d.card.kind == CardKind::Person => {
            d.memberships.iter().map(|m| m.title.clone()).collect()
        }
        ExportField::Position if d.card.kind == CardKind::Person => {
            d.memberships.iter().map(|m| m.position.clone()).filter(|p| !p.is_empty()).collect()
        }
        _ => vec![],
    }
}

/// Тексты ячеек одной строки. Поле в одном столбце — все значения через
/// запятую; в нескольких («Телефон 1», «Телефон 2») — по одному по порядку,
/// а остаток, если значений больше столбцов, — в последний.
pub fn row_cells(d: &CardDetails, mapping: &[Option<ExportField>], labels: &KindLabels) -> Vec<String> {
    let mut columns_of: HashMap<ExportField, usize> = HashMap::new();
    for f in mapping.iter().flatten() {
        *columns_of.entry(*f).or_default() += 1;
    }
    let mut seen: HashMap<ExportField, usize> = HashMap::new();
    let mut cache: HashMap<ExportField, Vec<String>> = HashMap::new();
    mapping
        .iter()
        .map(|f| {
            let Some(f) = *f else { return String::new() };
            let values = cache.entry(f).or_insert_with(|| field_values(d, f, labels));
            let total = columns_of[&f];
            let idx = seen.entry(f).or_default();
            let text = if total == 1 {
                values.join(", ")
            } else if *idx + 1 < total {
                values.get(*idx).cloned().unwrap_or_default()
            } else {
                values.get(*idx..).map(|rest| rest.join(", ")).unwrap_or_default()
            };
            *idx += 1;
            text
        })
        .collect()
}

#[derive(Debug)]
pub enum ExportError {
    SameAsTemplate,
    Read(String),
    NoSheet(String),
    Write(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::SameAsTemplate => write!(f, "output must differ from the template"),
            ExportError::Read(e) => write!(f, "cannot read template: {e}"),
            ExportError::NoSheet(s) => write!(f, "sheet not found: {s}"),
            ExportError::Write(e) => write!(f, "cannot write file: {e}"),
        }
    }
}

/// Заполняет копию образца. `header_row` — индекс строки заголовков с 0
/// (как в чтении таблиц); данные идут со следующей строки. Образец не
/// меняется: результат пишется в `output`.
pub fn write_export(
    template: &Path,
    sheet_name: &str,
    header_row: u32,
    mapping: &[Option<ExportField>],
    rows: &[Vec<String>],
    output: &Path,
) -> Result<usize, ExportError> {
    let same = match (template.canonicalize(), output.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => template == output,
    };
    if same {
        return Err(ExportError::SameAsTemplate);
    }
    let mut book = umya_spreadsheet::reader::xlsx::read(template).map_err(|e| ExportError::Read(e.to_string()))?;
    let sheet =
        book.sheet_by_name_mut(sheet_name).map_err(|_| ExportError::NoSheet(sheet_name.to_string()))?;

    // Координаты umya — (столбец, строка) с 1.
    let first = header_row + 2;
    let columns: Vec<u32> =
        mapping.iter().enumerate().filter(|(_, f)| f.is_some()).map(|(i, _)| i as u32 + 1).collect();
    // Оформление первой строки данных образца — для всех выгружаемых строк и
    // всех столбцов шапки, включая незаполняемые: иначе заливка строки
    // обрывалась бы на последнем заполняемом столбце.
    let all_columns: Vec<u32> = (1..=mapping.len() as u32).collect();
    let styles: HashMap<u32, umya_spreadsheet::Style> =
        all_columns.iter().map(|&c| (c, sheet.style((c, first)).clone())).collect();
    // В образце под заголовками могли остаться старые значения — стереть их
    // в заполняемых столбцах, чтобы не смешались с новыми.
    let highest = sheet.highest_row();
    for r in first..=highest.max(first) {
        for &c in &columns {
            sheet.cell_mut((c, r)).set_value_string("");
        }
    }
    for (i, cells) in rows.iter().enumerate() {
        let r = first + i as u32;
        for &c in &columns {
            let text = cells.get(c as usize - 1).cloned().unwrap_or_default();
            // Строго как текст: «79001234567» не должно стать числом 7,9E+10.
            sheet.cell_mut((c, r)).set_value_string(text);
        }
        if i > 0 {
            for &c in &all_columns {
                sheet.set_style((c, r), styles[&c].clone());
            }
        }
    }
    umya_spreadsheet::writer::xlsx::write(&book, output).map_err(|e| ExportError::Write(e.to_string()))?;
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Address, Card, Contact, Membership, Term};
    use crate::spreadsheet::read_sheets;

    fn labels() -> KindLabels {
        KindLabels { company: "Компания".into(), person: "Человек".into() }
    }

    fn person() -> CardDetails {
        let contact = |id, channel, value: &str| Contact { id, channel, value: value.into(), label: String::new() };
        CardDetails {
            card: Card {
                id: 1,
                kind: CardKind::Person,
                title: "Иван Тестов".into(),
                note: "заметка".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
            roles: vec![Term { id: 1, name: "Монтажник".into(), is_preset: true }],
            specializations: vec![],
            contacts: vec![
                contact(1, Channel::Phone, "79000001111"),
                contact(2, Channel::Telegram, "@ivan_test"),
                contact(3, Channel::Phone, "79000002222"),
                contact(4, Channel::Phone, "79000003333"),
            ],
            addresses: vec![Address {
                id: 1,
                label: Some(Term { id: 2, name: "Склад".into(), is_preset: true }),
                text: "ул. Примерная, 1".into(),
            }],
            file_links: vec![],
            memberships: vec![Membership { card_id: 9, title: "Ателье".into(), position: "Бригадир".into() }],
        }
    }

    #[test]
    fn one_column_joins_values_several_columns_split_them() {
        use ExportField::*;
        let d = person();
        assert_eq!(row_cells(&d, &[Some(Title), Some(Phone)], &labels()), ["Иван Тестов", "79000001111, 79000002222, 79000003333"]);
        // Два столбца на три телефона: первый — первый номер, последний — остаток.
        assert_eq!(
            row_cells(&d, &[Some(Phone), None, Some(Phone)], &labels()),
            ["79000001111", "", "79000002222, 79000003333"]
        );
        assert_eq!(
            row_cells(&d, &[Some(Phone), Some(Phone), Some(Phone), Some(Phone)], &labels()),
            ["79000001111", "79000002222", "79000003333", ""]
        );
    }

    #[test]
    fn fields_for_person() {
        use ExportField::*;
        let d = person();
        let cells = row_cells(
            &d,
            &[Some(Kind), Some(Telegram), Some(Address), Some(Roles), Some(Company), Some(Position), Some(Note)],
            &labels(),
        );
        assert_eq!(cells, ["Человек", "@ivan_test", "Склад: ул. Примерная, 1", "Монтажник", "Ателье", "Бригадир", "заметка"]);
    }

    #[test]
    fn writes_into_copy_of_template_keeping_header_and_text_phones() {
        let dir = tempfile::tempdir().unwrap();
        let template = dir.path().join("образец.xlsx");
        let output = dir.path().join("выгрузка.xlsx");
        {
            let mut book = rust_xlsxwriter::Workbook::new();
            let s = book.add_worksheet().set_name("Лист1").unwrap();
            s.write_string(0, 0, "Мой список").unwrap();
            s.write_string(1, 0, "Имя").unwrap();
            s.write_string(1, 1, "Телефон 1").unwrap();
            s.write_string(1, 2, "Телефон 2").unwrap();
            s.write_string(1, 3, "Моё поле").unwrap();
            s.write_string(2, 0, "старая строка").unwrap();
            s.write_string(2, 3, "не трогать").unwrap();
            book.save(&template).unwrap();
        }
        let mapping = [Some(ExportField::Title), Some(ExportField::Phone), Some(ExportField::Phone), None];
        let d = person();
        let rows = vec![row_cells(&d, &mapping, &labels()), row_cells(&d, &mapping, &labels())];
        assert_eq!(write_export(&template, "Лист1", 1, &mapping, &rows, &output).unwrap(), 2);

        let out = &read_sheets(&output).unwrap()[0];
        assert_eq!(out.rows[0], ["Мой список"]);
        assert_eq!(out.rows[1], ["Имя", "Телефон 1", "Телефон 2", "Моё поле"]);
        // Старое значение в заполняемом столбце стёрто, в чужом — осталось.
        assert_eq!(out.rows[2], ["Иван Тестов", "79000001111", "79000002222, 79000003333", "не трогать"]);
        assert_eq!(out.rows[3][0], "Иван Тестов");

        // Образец не изменился.
        let tpl = &read_sheets(&template).unwrap()[0];
        assert_eq!(tpl.rows[2][0], "старая строка");
    }

    #[test]
    fn phones_stay_text_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let template = dir.path().join("t.xlsx");
        let output = dir.path().join("o.xlsx");
        {
            let mut book = rust_xlsxwriter::Workbook::new();
            book.add_worksheet().write_string(0, 0, "Телефон").unwrap();
            book.save(&template).unwrap();
        }
        let mapping = [Some(ExportField::Phone)];
        write_export(&template, "Sheet1", 0, &mapping, &[vec!["79000001111".into()]], &output).unwrap();
        let book = umya_spreadsheet::reader::xlsx::read(&output).unwrap();
        let cell = book.sheet_by_name("Sheet1").unwrap().cell((1, 2)).unwrap();
        assert_eq!(cell.data_type(), "s", "phone must be stored as a string, not a number");
    }

    #[test]
    fn first_data_row_style_is_copied_to_all_columns_including_unmapped() {
        let dir = tempfile::tempdir().unwrap();
        let template = dir.path().join("t.xlsx");
        let output = dir.path().join("o.xlsx");
        {
            let fill = rust_xlsxwriter::Format::new().set_background_color(rust_xlsxwriter::Color::RGB(0xFFF4D6));
            let mut book = rust_xlsxwriter::Workbook::new();
            let s = book.add_worksheet();
            s.write_string(0, 0, "Имя").unwrap();
            s.write_string(0, 1, "Моё поле").unwrap();
            s.write_blank(1, 0, &fill).unwrap();
            s.write_blank(1, 1, &fill).unwrap();
            book.save(&template).unwrap();
        }
        let mapping = [Some(ExportField::Title), None];
        let rows: Vec<Vec<String>> = (0..3).map(|i| vec![format!("Имя {i}"), String::new()]).collect();
        write_export(&template, "Sheet1", 0, &mapping, &rows, &output).unwrap();

        let book = umya_spreadsheet::reader::xlsx::read(&output).unwrap();
        let sheet = book.sheet_by_name("Sheet1").unwrap();
        for col in [1u32, 2] {
            let first = sheet.style((col, 2));
            assert!(first.background_color().is_some(), "template fill lost in column {col}");
            for row in [3u32, 4] {
                assert_eq!(sheet.style((col, row)), first, "style not copied to ({col}, {row})");
            }
        }
    }

    #[test]
    fn refuses_to_overwrite_template_and_reports_missing_sheet() {
        let dir = tempfile::tempdir().unwrap();
        let template = dir.path().join("t.xlsx");
        {
            let mut book = rust_xlsxwriter::Workbook::new();
            book.add_worksheet().write_string(0, 0, "Имя").unwrap();
            book.save(&template).unwrap();
        }
        let mapping = [Some(ExportField::Title)];
        assert!(matches!(
            write_export(&template, "Sheet1", 0, &mapping, &[], &template),
            Err(ExportError::SameAsTemplate)
        ));
        let out = dir.path().join("o.xlsx");
        assert!(matches!(write_export(&template, "Нет", 0, &mapping, &[], &out), Err(ExportError::NoSheet(_))));
    }
}
