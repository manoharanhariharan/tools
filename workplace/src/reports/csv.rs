//! CSV exports. Values are plain decimals (no currency symbol) so they open
//! cleanly in spreadsheets; a UTF-8 BOM is prepended for Excel compatibility.

use anyhow::Context;

use crate::domain::money::{format_decimal, format_hours};
use crate::reports::summary::{MoneyTotals, ReportData, ReportLayout};

pub fn render(data: &ReportData, layout: ReportLayout) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());

    match layout {
        ReportLayout::Employee => {
            writer.write_record([
                "employee",
                "code",
                "pay_basis",
                "entries",
                "regular_hours",
                "overtime_hours",
                "base",
                "overtime",
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
                    employee.pay_basis.as_str(),
                    &t.entries.to_string(),
                    &format_hours(t.regular_minutes),
                    &format_hours(t.overtime_minutes),
                    &format_decimal(t.base_minor),
                    &format_decimal(t.ot_minor),
                    &format_decimal(t.bonus_minor),
                    &format_decimal(t.gross_minor),
                    &format_decimal(t.advance_minor),
                    &format_decimal(t.deduction_minor),
                    &format_decimal(t.net_minor),
                ])?;
            }
            write_total_row(&mut writer, &data.totals, &["TOTAL", "", ""])?;
        }
        ReportLayout::Daily => {
            writer.write_record([
                "date",
                "entries",
                "regular_hours",
                "overtime_hours",
                "base",
                "overtime",
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
                    &format_hours(t.regular_minutes),
                    &format_hours(t.overtime_minutes),
                    &format_decimal(t.base_minor),
                    &format_decimal(t.ot_minor),
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
                "pay_basis",
                "day_fraction",
                "regular_hours",
                "overtime_hours",
                "base",
                "overtime",
                "bonus",
                "gross",
                "advance",
                "deduction",
                "net",
                "note",
            ])?;
            for line in &data.lines {
                writer.write_record([
                    line.work_date.as_str(),
                    line.employee_name.as_str(),
                    line.employee_code.as_deref().unwrap_or(""),
                    line.employee_pay_basis.as_str(),
                    &super::summary::fraction_label(line.day_fraction_bp),
                    &format_hours(line.regular_minutes),
                    &format_hours(line.overtime_minutes),
                    &format_decimal(line.base_minor),
                    &format_decimal(line.ot_minor),
                    &format_decimal(line.bonus_minor),
                    &format_decimal(line.gross_minor),
                    &format_decimal(line.advance_minor),
                    &format_decimal(line.deduction_minor),
                    &format_decimal(line.net_minor),
                    line.note.as_str(),
                ])?;
            }
            let t = &data.totals;
            writer.write_record([
                "TOTAL",
                "",
                "",
                "",
                "",
                &format_hours(t.regular_minutes),
                &format_hours(t.overtime_minutes),
                &format_decimal(t.base_minor),
                &format_decimal(t.ot_minor),
                &format_decimal(t.bonus_minor),
                &format_decimal(t.gross_minor),
                &format_decimal(t.advance_minor),
                &format_decimal(t.deduction_minor),
                &format_decimal(t.net_minor),
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

/// Totals row: `prefix` fills the leading text columns, then the standard
/// money/hour columns follow.
fn write_total_row<W: std::io::Write>(
    writer: &mut csv::Writer<W>,
    totals: &MoneyTotals,
    prefix: &[&str],
) -> anyhow::Result<()> {
    let mut record: Vec<String> = prefix.iter().map(|value| value.to_string()).collect();
    record.push(totals.entries.to_string());
    record.push(format_hours(totals.regular_minutes));
    record.push(format_hours(totals.overtime_minutes));
    record.push(format_decimal(totals.base_minor));
    record.push(format_decimal(totals.ot_minor));
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
    use crate::repo::entries::EntryRow;

    fn sample_data() -> ReportData {
        let line = EntryRow {
            id: "1".into(),
            employee_id: "e1".into(),
            work_date: "2026-11-01".into(),
            day_fraction_bp: 10_000,
            regular_minutes: 450,
            overtime_minutes: 60,
            ot_multiplier_bp: None,
            override_amount_minor: None,
            bonus_minor: 10_000,
            advance_minor: 5_000,
            deduction_minor: 0,
            base_minor: 80_000,
            ot_minor: 15_000,
            gross_minor: 105_000,
            net_minor: 100_000,
            note: "note, with \"quotes\"".into(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            seq: 1,
            employee_name: "Asha, Kumari".into(),
            employee_code: Some("A-1".into()),
            employee_pay_basis: "day_rate".into(),
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
        assert_eq!(rows[1][3], "1");
        assert_eq!(rows[1][4], "7.50");
        assert_eq!(rows[1][6], "800.00");
        assert_eq!(rows[1][12], "1000.00");
        assert_eq!(rows.last().unwrap()[0], "TOTAL");
        assert_eq!(rows.last().unwrap()[12], "1000.00");
    }

    #[test]
    fn entries_csv_quotes_commas_and_quotes_and_formats_note() {
        let bytes = render(&sample_data(), ReportLayout::Entries).unwrap();
        let rows = parse(&bytes);
        assert_eq!(rows[0][0], "date");
        assert_eq!(rows[1][1], "Asha, Kumari");
        assert_eq!(rows[1][14], "note, with \"quotes\"");
        assert_eq!(rows[1][4], "1.00");
        assert_eq!(rows[1][13], "1000.00");
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
        assert_eq!(rows[1][10], "1000.00");
    }

    #[test]
    fn empty_report_still_has_headers_and_zero_totals() {
        let data = crate::reports::summary::build("2026-11-01", "2026-11-30", 0, Vec::new());
        let rows = parse(&render(&data, ReportLayout::Employee).unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][0], "TOTAL");
        assert_eq!(rows[1][12], "0.00");
    }
}
