//! Образец для ручной проверки экспорта (без данных). Файл кладётся в
//! `.incoming/` (вне git).
//!
//! cargo run --example make_export_template -- ../.incoming/sample_export_template.xlsx

use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, XlsxError};

fn main() -> Result<(), XlsxError> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "sample_export_template.xlsx".into());
    let title = Format::new().set_bold().set_font_size(14);
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xDCE6F1)).set_border(FormatBorder::Thin);
    // Оформление первой строки данных — должно повториться на всех строках выгрузки.
    let data = Format::new().set_background_color(Color::RGB(0xFFF4D6)).set_border(FormatBorder::Thin);

    let mut book = Workbook::new();
    let s = book.add_worksheet().set_name("Контакты")?;
    s.write_string_with_format(0, 0, "Мой список подрядчиков (образец)", &title)?;
    let headers = ["Название", "Вид", "Телефон 1", "Телефон 2", "Адрес", "Специализации", "Примечание", "Моё поле"];
    let widths = [26.0, 12.0, 18.0, 18.0, 34.0, 24.0, 28.0, 14.0];
    for (c, (h, w)) in headers.iter().zip(widths).enumerate() {
        s.write_string_with_format(2, c as u16, *h, &header)?;
        s.write_blank(3, c as u16, &data)?;
        s.set_column_width(c as u16, w)?;
    }
    book.save(&path)?;
    println!("saved {path}");
    Ok(())
}
