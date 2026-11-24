//! CSV exports. Values are plain decimals (no currency symbol) so they open
//! cleanly in spreadsheets; a UTF-8 BOM is prepended for Excel compatibility.

use anyhow::Context;

use crate::domain::money::{format_decimal, format_inr};
use crate::reports::summary::{MoneyTotals, ReportData, ReportLayout};

pub fn render(data: &ReportData, layout: ReportLayout) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());

    match layout {
        ReportLayout::Sunday => {
            writer.write_record([
                "sl_no",
                "name",
                "previous_total",
                "sunday_earning",
                "current_total",
            ])?;
            for (index, row) in data.ledger.iter().enumerate() {
                let sl_no = (index + 1).to_string();
                let previous = format_decimal(row.previous_minor);
                let sunday = format_decimal(row.sunday_minor);
                let current = format_decimal(row.current_minor);
                writer.write_record([
                    sl_no.as_str(),
                    row.name.as_str(),
                    previous.as_str(),
                    sunday.as_str(),
                    current.as_str(),
                ])?;
            }
            let previous = format_decimal(data.previous_total_minor);
            let sunday = format_decimal(data.sunday_total_minor);
            let current = format_decimal(data.current_total_minor);
            writer.write_record([
                "",
                "TOTAL",
                previous.as_str(),
                sunday.as_str(),
                current.as_str(),
            ])?;
        }
        ReportLayout::Employee => {
            writer.write_record([
                "employee",
                "code",
                "entries",
                "business",
                "bonus",
                "gross",
                "advance",
                "deduction",
                "net",
            ])?;
            for employee in &data.employees {
                let t = &employee.totals;
                writer.write_record([
                    employee.name.as_str(),
                    employee.code.as_deref().unwrap_or(""),
                    &t.entries.to_string(),
                    &format_decimal(t.business_minor),
                    &format_decimal(t.bonus_minor),
                    &format_decimal(t.gross_minor),
                    &format_decimal(t.advance_minor),
                    &format_decimal(t.deduction_minor),
                    &format_decimal(t.net_minor),
                ])?;
            }
            write_total_row(&mut writer, &data.totals, &["TOTAL", ""])?;
        }
        ReportLayout::Daily => {
            writer.write_record([
                "date",
                "entries",
                "business",
                "bonus",
                "gross",
                "advance",
                "deduction",
                "net",
            ])?;
            for day in &data.days {
                let t = &day.totals;
                writer.write_record([
                    day.work_date.as_str(),
                    &t.entries.to_string(),
                    &format_decimal(t.business_minor),
                    &format_decimal(t.bonus_minor),
                    &format_decimal(t.gross_minor),
                    &format_decimal(t.advance_minor),
                    &format_decimal(t.deduction_minor),
                    &format_decimal(t.net_minor),
                ])?;
            }
            write_total_row(&mut writer, &data.totals, &["TOTAL"])?;
        }
        ReportLayout::Entries => {
            writer.write_record([
                "date",
                "employee",
                "code",
                "business",
                "bonus",
                "gross",
                "advance",
                "deduction",
                "net",
                "details",
                "note",
            ])?;
            for line in &data.lines {
                let details = line
                    .lines
                    .iter()
                    .map(|entry_line| {
                        let mut text = format!(
                            "{}: {}",
                            entry_line.type_name,
                            format_inr(entry_line.amount_minor)
                        );
                        if !entry_line.description.is_empty() {
                            text.push_str(&format!(" ({})", entry_line.description));
                        }
                        text
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                writer.write_record([
                    line.work_date.as_str(),
                    line.employee_name.as_str(),
                    line.employee_code.as_deref().unwrap_or(""),
                    &format_decimal(line.business_minor),
                    &format_decimal(line.bonus_minor),
                    &format_decimal(line.gross_minor),
                    &format_decimal(line.advance_minor),
                    &format_decimal(line.deduction_minor),
                    &format_decimal(line.net_minor),
                    &details,
                    line.note.as_str(),
                ])?;
            }
            let t = &data.totals;
            writer.write_record([
                "TOTAL",
                "",
                "",
                &format_decimal(t.business_minor),
                &format_decimal(t.bonus_minor),
                &format_decimal(t.gross_minor),
                &format_decimal(t.advance_minor),
                &format_decimal(t.deduction_minor),
                &format_decimal(t.net_minor),
                "",
                "",
            ])?;
        }
    }

    writer.flush().context("cannot flush CSV")?;
    let mut bytes = writer
        .into_inner()
        .map_err(|err| anyhow::anyhow!("cannot finish CSV: {}", err.error()))?;
    let mut output = Vec::with_capacity(bytes.len() + 3);
    output.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    output.append(&mut bytes);
    Ok(output)
}

/// Totals row: `prefix` fills the leading text columns, then the standard money
/// columns follow (entries, business, bonus, gross, advance, deduction, net).
fn write_total_row<W: std::io::Write>(
    writer: &mut csv::Writer<W>,
    totals: &MoneyTotals,
    prefix: &[&str],
) -> anyhow::Result<()> {
    let mut record: Vec<String> = prefix.iter().map(|value| value.to_string()).collect();
    record.push(totals.entries.to_string());
    record.push(format_decimal(totals.business_minor));
    record.push(format_decimal(totals.bonus_minor));
    record.push(format_decimal(totals.gross_minor));
    record.push(format_decimal(totals.advance_minor));
    record.push(format_decimal(totals.deduction_minor));
    record.push(format_decimal(totals.net_minor));
    writer.write_record(record)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::entries::{EntryLineRow, EntryRow};

    fn sample_data() -> ReportData {
        let line = EntryRow {
            id: "1".into(),
            employee_id: "e1".into(),
            work_date: "2026-11-01".into(),
            business_minor: 80_000,
            bonus_minor: 10_000,
            advance_minor: 5_000,
            deduction_minor: 0,
            gross_minor: 90_000,
            net_minor: 85_000,
            note: "note, with \"quotes\"".into(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            seq: 1,
            employee_name: "Asha, Kumari".into(),
            employee_code: Some("A-1".into()),
            lines: vec![EntryLineRow {
                id: "l1".into(),
                entry_id: "1".into(),
                type_id: "sales".into(),
                type_name: "Sales".into(),
                description: "3 jobs".into(),
                amount_minor: 80_000,
                position: 0,
                created_at: 0,
            }],
        };
        crate::reports::summary::build("2026-11-01", "2026-11-30", 0, vec![line])
    }

    fn parse(bytes: &[u8]) -> Vec<Vec<String>> {
        let text = std::str::from_utf8(&bytes[3..]).expect("utf-8 after BOM");
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(text.as_bytes());
        reader
            .records()
            .map(|r| r.unwrap().iter().map(str::to_string).collect())
            .collect()
    }

    #[test]
    fn employee_csv_has_bom_headers_and_reconciled_totals() {
        let bytes = render(&sample_data(), ReportLayout::Employee).unwrap();
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
        let rows = parse(&bytes);
        assert_eq!(rows[0][0], "employee");
        assert_eq!(rows[1][0], "Asha, Kumari");
        assert_eq!(rows[1][2], "1");
        assert_eq!(rows[1][3], "800.00");
        assert_eq!(rows[1][4], "100.00");
        assert_eq!(rows[1][8], "850.00");
        assert_eq!(rows.last().unwrap()[0], "TOTAL");
        assert_eq!(rows.last().unwrap()[8], "850.00");
    }

    #[test]
    fn entries_csv_quotes_commas_and_lists_details() {
        let bytes = render(&sample_data(), ReportLayout::Entries).unwrap();
        let rows = parse(&bytes);
        assert_eq!(rows[0][0], "date");
        assert_eq!(rows[1][1], "Asha, Kumari");
        assert!(rows[1][9].contains("Sales: ₹800.00 (3 jobs)"));
        assert_eq!(rows[1][10], "note, with \"quotes\"");
        assert_eq!(rows[1][8], "850.00");
        assert_eq!(rows.last().unwrap()[0], "TOTAL");
        assert_eq!(rows.last().unwrap()[1], "");
    }

    #[test]
    fn daily_csv_lists_each_day() {
        let bytes = render(&sample_data(), ReportLayout::Daily).unwrap();
        let rows = parse(&bytes);
        assert_eq!(rows[0][0], "date");
        assert_eq!(rows[1][0], "2026-11-01");
        assert_eq!(rows[1][1], "1");
        assert_eq!(rows[1][7], "850.00");
    }

    #[test]
    fn sunday_csv_uses_ledger_columns_and_totals() {
        let bytes = render(&sample_data(), ReportLayout::Sunday).unwrap();
        let rows = parse(&bytes);
        assert_eq!(rows[0][0], "sl_no");
        assert_eq!(rows[0][3], "sunday_earning");
        assert_eq!(rows[1][0], "1");
        assert_eq!(rows[1][1], "Asha, Kumari");
        assert_eq!(rows[1][2], "0.00");
        assert_eq!(rows[1][3], "850.00");
        assert_eq!(rows[1][4], "850.00");
        assert_eq!(rows[2][1], "TOTAL");
        assert_eq!(rows[2][4], "850.00");
    }

    #[test]
    fn empty_report_still_has_headers_and_zero_totals() {
        let data = crate::reports::summary::build("2026-11-01", "2026-11-30", 0, Vec::new());
        let rows = parse(&render(&data, ReportLayout::Employee).unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][0], "TOTAL");
        assert_eq!(rows[1][8], "0.00");
    }
}
