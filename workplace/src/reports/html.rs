//! Printable HTML report. Self-contained (inline CSS, no scripts) so it can be
//! opened offline, emailed, or printed to PDF from any browser.

use crate::domain::date;
use crate::domain::money::format_inr;
use crate::reports::summary::{MoneyTotals, ReportData, ReportLayout};

pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn display_date(value: &str) -> String {
    date::parse_iso(value)
        .map(date::display)
        .unwrap_or_else(|| value.to_string())
}

fn line_detail(line: &crate::repo::entries::EntryRow) -> String {
    line.lines
        .iter()
        .map(|entry_line| {
            let mut text = format!(
                "{} {}",
                entry_line.type_name,
                format_inr(entry_line.amount_minor)
            );
            if !entry_line.description.is_empty() {
                text.push_str(&format!(" — {}", entry_line.description));
            }
            text
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn render(data: &ReportData, layout: ReportLayout) -> String {
    let title = format!(
        "Daybook earnings {} – {}",
        display_date(&data.from),
        display_date(&data.to)
    );
    let generated = date::format_datetime_ist(data.generated_at);

    let mut html = String::with_capacity(16 * 1024);
    html.push_str("<!doctype html>\n<html lang=\"en-IN\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<meta name=\"robots\" content=\"noindex\">\n");
    html.push_str(&format!("<title>{}</title>\n", escape(&title)));
    html.push_str(STYLE);
    html.push_str("</head>\n<body>\n");

    html.push_str("<header class=\"report-head\">");
    html.push_str("<div><h1>Employee business &amp; earnings</h1>");
    html.push_str(&format!(
        "<p class=\"period\">{} &rarr; {} <span class=\"sep\">&middot;</span> {} day sheets <span class=\"sep\">&middot;</span> Generated {} IST</p></div>",
        escape(&display_date(&data.from)),
        escape(&display_date(&data.to)),
        data.totals.entries,
        escape(&generated)
    ));
    html.push_str("</header>\n");

    html.push_str("<section class=\"cards\" aria-label=\"Period totals\">");
    card(
        &mut html,
        "Business generated",
        &format_inr(data.totals.business_minor),
        "",
    );
    card(
        &mut html,
        "Bonus",
        &format_inr(data.totals.bonus_minor),
        "added",
    );
    card(
        &mut html,
        "Advances",
        &format_inr(data.totals.advance_minor),
        "deducted",
    );
    card(
        &mut html,
        "Deductions",
        &format_inr(data.totals.deduction_minor),
        "deducted",
    );
    card(
        &mut html,
        "Final money",
        &format_inr(data.totals.net_minor),
        "to present",
    );
    card(
        &mut html,
        "Employees",
        &data.employees.len().to_string(),
        "",
    );
    html.push_str("</section>\n");

    html.push_str("<main>\n");
    match layout {
        ReportLayout::Employee => employee_table(&mut html, data),
        ReportLayout::Daily => daily_table(&mut html, data),
        ReportLayout::Entries => entries_table(&mut html, data),
    }
    type_table(&mut html, data);
    html.push_str("</main>\n");

    html.push_str(&format!(
        "<footer><p>Daybook &middot; amounts in Indian Rupees (INR) &middot; generated {} IST &middot; use your browser's Print command (Ctrl+P) to save this report as PDF.</p></footer>\n",
        escape(&generated)
    ));
    html.push_str("</body>\n</html>\n");
    html
}

fn card(html: &mut String, label: &str, value: &str, hint: &str) {
    html.push_str("<div class=\"card\">");
    html.push_str(&format!("<span class=\"label\">{}</span>", escape(label)));
    html.push_str(&format!("<strong>{}</strong>", escape(value)));
    if !hint.is_empty() {
        html.push_str(&format!("<span class=\"hint\">{}</span>", escape(hint)));
    }
    html.push_str("</div>");
}

fn money_cell(html: &mut String, value: i64, class: &str) {
    html.push_str(&format!(
        "<td class=\"num {}\">{}</td>",
        class,
        escape(&format_inr(value))
    ));
}

fn money_cells(html: &mut String, totals: &MoneyTotals) {
    money_cell(html, totals.business_minor, "");
    money_cell(html, totals.bonus_minor, "");
    money_cell(html, totals.gross_minor, "");
    money_cell(html, totals.advance_minor, "");
    money_cell(html, totals.deduction_minor, "");
    money_cell(html, totals.net_minor, "net");
}

fn employee_table(html: &mut String, data: &ReportData) {
    html.push_str("<h2>By employee</h2>\n");
    html.push_str("<table>\n<thead><tr>");
    for header in [
        "Employee",
        "Code",
        "Entries",
        "Business",
        "Bonus",
        "Gross",
        "Advance",
        "Deduction",
        "Final money",
    ] {
        html.push_str(&format!(
            "<th scope=\"col\"{}>{header}</th>",
            if header == "Employee" || header == "Code" {
                ""
            } else {
                " class=\"num\""
            }
        ));
    }
    html.push_str("</tr></thead>\n<tbody>\n");

    if data.employees.is_empty() {
        empty_row(html, 9, "No day sheets in this period.");
    }
    for employee in &data.employees {
        html.push_str("<tr>");
        html.push_str(&format!(
            "<th scope=\"row\">{}</th>",
            escape(&employee.name)
        ));
        html.push_str(&format!(
            "<td>{}</td><td class=\"num\">{}</td>",
            escape(employee.code.as_deref().unwrap_or("—")),
            employee.totals.entries
        ));
        money_cells(html, &employee.totals);
        html.push_str("</tr>\n");
    }

    html.push_str("<tr class=\"total-row\">");
    html.push_str("<th scope=\"row\" colspan=\"2\">Total</th>");
    html.push_str(&format!("<td class=\"num\">{}</td>", data.totals.entries));
    money_cells(html, &data.totals);
    html.push_str("</tr>");
    html.push_str("</tbody>\n</table>\n");
}

fn daily_table(html: &mut String, data: &ReportData) {
    html.push_str("<h2>By day</h2>\n");
    html.push_str("<table>\n<thead><tr>");
    for header in [
        "Date",
        "Entries",
        "Business",
        "Bonus",
        "Gross",
        "Advance",
        "Deduction",
        "Final money",
    ] {
        html.push_str(&format!(
            "<th scope=\"col\"{}>{header}</th>",
            if header == "Date" {
                ""
            } else {
                " class=\"num\""
            }
        ));
    }
    html.push_str("</tr></thead>\n<tbody>\n");

    if data.days.is_empty() {
        empty_row(html, 8, "No day sheets in this period.");
    }
    for day in &data.days {
        html.push_str("<tr>");
        html.push_str(&format!(
            "<th scope=\"row\">{}</th>",
            escape(&display_date(&day.work_date))
        ));
        html.push_str(&format!("<td class=\"num\">{}</td>", day.totals.entries));
        money_cells(html, &day.totals);
        html.push_str("</tr>\n");
    }

    html.push_str("<tr class=\"total-row\">");
    html.push_str("<th scope=\"row\">Total</th>");
    html.push_str(&format!("<td class=\"num\">{}</td>", data.totals.entries));
    money_cells(html, &data.totals);
    html.push_str("</tr>");
    html.push_str("</tbody>\n</table>\n");
}

fn entries_table(html: &mut String, data: &ReportData) {
    html.push_str("<h2>Day sheets</h2>\n");
    html.push_str("<table class=\"detail\">\n<thead><tr>");
    for header in [
        "Date",
        "Employee",
        "Business",
        "Bonus",
        "Gross",
        "Advance",
        "Deduction",
        "Final money",
        "Earning detail",
        "Note",
    ] {
        html.push_str(&format!(
            "<th scope=\"col\"{}>{header}</th>",
            if header == "Date"
                || header == "Employee"
                || header == "Earning detail"
                || header == "Note"
            {
                ""
            } else {
                " class=\"num\""
            }
        ));
    }
    html.push_str("</tr></thead>\n<tbody>\n");

    if data.lines.is_empty() {
        empty_row(html, 10, "No day sheets in this period.");
    }
    for line in &data.lines {
        html.push_str("<tr>");
        html.push_str(&format!(
            "<th scope=\"row\">{}</th>",
            escape(&display_date(&line.work_date))
        ));
        html.push_str(&format!("<td>{}</td>", escape(&line.employee_name)));
        money_cells(
            html,
            &MoneyTotals {
                entries: 1,
                business_minor: line.business_minor,
                bonus_minor: line.bonus_minor,
                gross_minor: line.gross_minor,
                advance_minor: line.advance_minor,
                deduction_minor: line.deduction_minor,
                net_minor: line.net_minor,
            },
        );
        html.push_str(&format!(
            "<td class=\"deet\">{}</td><td class=\"note\">{}</td>",
            escape(&line_detail(line)),
            escape(&line.note)
        ));
        html.push_str("</tr>\n");
    }

    html.push_str("<tr class=\"total-row\">");
    html.push_str("<th scope=\"row\" colspan=\"2\">Total</th>");
    money_cells(html, &data.totals);
    html.push_str("<td></td><td></td>");
    html.push_str("</tr>");
    html.push_str("</tbody>\n</table>\n");
}

fn type_table(html: &mut String, data: &ReportData) {
    html.push_str("<h2>Business by earning type</h2>\n");
    html.push_str("<table class=\"types\">\n<thead><tr>");
    html.push_str("<th scope=\"col\">Type</th><th scope=\"col\" class=\"num\">Business</th>");
    html.push_str("</tr></thead>\n<tbody>\n");
    if data.types.is_empty() {
        empty_row(html, 2, "No business recorded in this period.");
    }
    for entry_type in &data.types {
        html.push_str("<tr>");
        html.push_str(&format!(
            "<th scope=\"row\">{}</th>",
            escape(&entry_type.type_name)
        ));
        money_cell(html, entry_type.amount_minor, "");
        html.push_str("</tr>\n");
    }
    html.push_str("<tr class=\"total-row\">");
    html.push_str("<th scope=\"row\">Total business</th>");
    money_cell(html, data.totals.business_minor, "net");
    html.push_str("</tr>");
    html.push_str("</tbody>\n</table>\n");
}

fn empty_row(html: &mut String, columns: usize, message: &str) {
    html.push_str(&format!(
        "<tr class=\"empty\"><td colspan=\"{columns}\">{}</td></tr>\n",
        escape(message)
    ));
}

const STYLE: &str = r#"<style>
:root { color-scheme: light; }
* { box-sizing: border-box; }
body { font-family: "Segoe UI", Roboto, Helvetica, Arial, sans-serif; color: #17202a;
       margin: 0; padding: 24px; background: #fff; font-size: 13px; }
h1 { font-size: 22px; margin: 0 0 4px; letter-spacing: -0.2px; }
h2 { font-size: 15px; margin: 24px 0 8px; }
.period { margin: 0; color: #5b6b7c; font-size: 12px; }
.sep { margin: 0 6px; color: #b6c2cf; }
.cards { display: flex; flex-wrap: wrap; gap: 10px; margin: 18px 0 4px; }
.card { border: 1px solid #dde3ea; border-radius: 8px; padding: 10px 14px; min-width: 132px; }
.card .label { display: block; font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: #6b7c8e; }
.card strong { display: block; font-size: 15px; margin-top: 2px; font-variant-numeric: tabular-nums; }
.card .hint { display: block; font-size: 10px; color: #8a97a6; margin-top: 2px; }
table { width: 100%; border-collapse: collapse; margin: 0 0 8px; }
th, td { padding: 6px 8px; border-bottom: 1px solid #e3e8ee; text-align: left; vertical-align: top; }
thead th { background: #f4f6f9; font-size: 10px; text-transform: uppercase; letter-spacing: 0.05em;
           color: #526273; border-bottom: 1px solid #cfd8e3; }
td.num, th.num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
tbody tr:nth-child(even) { background: #fbfcfd; }
.total-row th, .total-row td { font-weight: 600; border-top: 2px solid #cfd8e3; background: #f4f6f9; }
.net { font-weight: 600; }
td.deet { color: #3d4c5c; max-width: 280px; }
td.note { color: #5b6b7c; max-width: 200px; }
tr.empty td { text-align: center; color: #8a97a6; padding: 28px; }
footer { margin-top: 26px; border-top: 1px solid #e3e8ee; padding-top: 10px; color: #7b8898; font-size: 11px; }
@media print {
  @page { size: A4; margin: 12mm; }
  body { padding: 0; font-size: 10.5px; }
  h1 { font-size: 17px; }
  h2 { font-size: 12px; margin: 14px 0 6px; }
  .cards { gap: 6px; }
  .card { border-color: #cfd8e3; padding: 6px 9px; min-width: 100px; }
  .card strong { font-size: 12px; }
  thead { display: table-header-group; }
  tr { break-inside: avoid; }
  footer { position: fixed; bottom: 0; left: 0; right: 0; border-top: none; }
}
</style>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::entries::{EntryLineRow, EntryRow};

    fn data(name: &str, detail: &str) -> ReportData {
        let line = EntryRow {
            id: "1".into(),
            employee_id: "e1".into(),
            work_date: "2026-11-01".into(),
            business_minor: 80_000,
            bonus_minor: 0,
            advance_minor: 20_000,
            deduction_minor: 0,
            gross_minor: 80_000,
            net_minor: 60_000,
            note: "day note".into(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            seq: 1,
            employee_name: name.into(),
            employee_code: Some("A-1".into()),
            lines: vec![EntryLineRow {
                id: "l1".into(),
                entry_id: "1".into(),
                type_id: "sales".into(),
                type_name: "Sales".into(),
                description: detail.into(),
                amount_minor: 80_000,
                position: 0,
                created_at: 0,
            }],
        };
        crate::reports::summary::build("2026-11-01", "2026-11-30", 1_700_000_000_000, vec![line])
    }

    #[test]
    fn escapes_untrusted_names_and_details() {
        let html = render(
            &data("<script>alert('x')</script>", "a & b \"c\""),
            ReportLayout::Entries,
        );
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"));
        assert!(html.contains("a &amp; b &quot;c&quot;"));
    }

    #[test]
    fn includes_totals_type_breakdown_and_print_styles() {
        let html = render(&data("Asha", "3 jobs"), ReportLayout::Employee);
        assert!(html.contains("₹800.00"));
        assert!(html.contains("₹200.00"));
        assert!(html.contains("₹600.00"));
        assert!(html.contains("@media print"));
        assert!(html.contains("Business by earning type"));
        assert!(html.contains("01-11-2026"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn each_layout_renders_its_own_table() {
        let data = data("Asha", "3 jobs");
        assert!(render(&data, ReportLayout::Employee).contains("By employee"));
        assert!(render(&data, ReportLayout::Daily).contains("By day"));
        assert!(render(&data, ReportLayout::Entries).contains("Day sheets"));
    }

    #[test]
    fn detail_totals_row_aligns_with_its_ten_columns() {
        let html = render(&data("Asha", "3 jobs"), ReportLayout::Entries);
        let totals = html
            .split("<tr class=\"total-row\">")
            .nth(1)
            .expect("totals row");
        let row = totals.split("</tr>").next().unwrap();
        assert_eq!(row.matches("<td").count(), 8);
        assert!(row.contains("colspan=\"2\""));
        assert!(row.contains("₹600.00"));
    }

    #[test]
    fn empty_data_shows_empty_state_not_a_broken_table() {
        let empty = crate::reports::summary::build("2026-11-01", "2026-11-30", 0, Vec::new());
        let html = render(&empty, ReportLayout::Employee);
        assert!(html.contains("No day sheets in this period."));
        assert!(html.contains("₹0.00"));
    }
}
