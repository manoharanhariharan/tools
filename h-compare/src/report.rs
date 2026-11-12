//! Headless and HTML report generation.

use crate::csvdiff::{CsvCompare, CsvRowKind};
use crate::textdiff::{char_range_to_bytes, TextDiffView};
use std::fmt::Write as _;

pub fn csv_differences(compare: &CsvCompare) -> String {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    let _ = writer.write_record([
        "status",
        "left_row",
        "right_row",
        "column",
        "left_value",
        "right_value",
    ]);
    for col in &compare.changed_headers {
        let _ = writer.write_record([
            "header".to_string(),
            String::new(),
            String::new(),
            compare.column_name(*col),
            compare.left.header(*col).to_string(),
            compare.right.header(*col).to_string(),
        ]);
    }
    for row in &compare.rows {
        let left_no = row
            .left
            .map(|idx| compare.left.row_number(idx).to_string())
            .unwrap_or_default();
        let right_no = row
            .right
            .map(|idx| compare.right.row_number(idx).to_string())
            .unwrap_or_default();
        match row.kind {
            CsvRowKind::Equal => {}
            CsvRowKind::Modified => {
                for cell in &row.cells {
                    let _ = writer.write_record([
                        "modified".to_string(),
                        left_no.clone(),
                        right_no.clone(),
                        compare.column_name(cell.col),
                        cell.left.clone(),
                        cell.right.clone(),
                    ]);
                }
            }
            CsvRowKind::LeftOnly => {
                let values = row
                    .left
                    .map(|idx| join_row(&compare.left.rows[idx]))
                    .unwrap_or_default();
                let _ = writer.write_record([
                    "removed".to_string(),
                    left_no,
                    right_no,
                    String::new(),
                    values,
                    String::new(),
                ]);
            }
            CsvRowKind::RightOnly => {
                let values = row
                    .right
                    .map(|idx| join_row(&compare.right.rows[idx]))
                    .unwrap_or_default();
                let _ = writer.write_record([
                    "added".to_string(),
                    left_no,
                    right_no,
                    String::new(),
                    String::new(),
                    values,
                ]);
            }
        }
    }
    let bytes = writer.into_inner().unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn join_row(row: &[String]) -> String {
    row.join(" | ")
}

pub fn csv_html_report(compare: &CsvCompare, left_name: &str, right_name: &str) -> String {
    let mut html = String::new();
    let stats = compare.stats;
    html.push_str("<!doctype html><html><head><meta charset=\"utf-8\">");
    let _ = write!(
        html,
        "<title>CSV comparison - {} vs {}</title>",
        escape_html(left_name),
        escape_html(right_name)
    );
    html.push_str(
        r#"<style>
:root{color-scheme:dark}body{font:13px/1.45 ui-sans-serif,system-ui,Segoe UI,sans-serif;background:#17191d;color:#dde1e8;margin:0;padding:24px}
h1{font-size:20px;margin:0 0 4px}h2{font-size:14px;margin:24px 0 8px;color:#aab3c0}
.sub{color:#9aa4b2;margin-bottom:18px}.stats{display:flex;gap:8px;flex-wrap:wrap;margin:14px 0}
.chip{border:1px solid #353b45;border-radius:999px;padding:4px 10px;background:#22262d}
.chip b{color:#fff}.wrap{overflow:auto;border:1px solid #303641;border-radius:8px;max-height:80vh}
table{border-collapse:separate;border-spacing:0;min-width:100%;font:12px/1.4 ui-monospace,Consolas,monospace}
th{position:sticky;top:0;z-index:2;background:#22262d;color:#cbd3df;text-align:left;padding:8px 10px;border-bottom:1px solid #3a414c;white-space:nowrap}
td{padding:5px 10px;border-bottom:1px solid #242932;vertical-align:top;white-space:pre-wrap;max-width:420px}
tr:hover td{background:#1e232b}.rn{color:#7f8a99;white-space:nowrap;width:1%}
.eq{color:#c7ced8}.mod td{background:#2a271a}.del td{background:#2b1d1f}.add td{background:#182720}
.changed{background:#3a2828!important}.changed-right{background:#1d3328!important}.hdr-changed{color:#ffcf70}
.a{color:#ff9b9b}.b{color:#9fe3b5}.note{color:#ffcf70;margin:8px 0}.legend{display:flex;gap:16px;color:#9aa4b2}
</style></head><body>"#,
    );
    let _ = write!(
        html,
        "<h1>CSV comparison</h1><div class=\"sub\"><span class=\"a\">{}</span> &harr; <span class=\"b\">{}</span></div>",
        escape_html(left_name),
        escape_html(right_name)
    );
    let _ = write!(
        html,
        "<div class=\"stats\"><span class=\"chip\">rows <b>{}</b> / <b>{}</b></span><span class=\"chip\">equal <b>{}</b></span><span class=\"chip\">modified <b>{}</b></span><span class=\"chip\">added <b>{}</b></span><span class=\"chip\">removed <b>{}</b></span><span class=\"chip\">changed cells <b>{}</b></span></div>",
        stats.left_rows,
        stats.right_rows,
        stats.equal,
        stats.modified,
        stats.added,
        stats.removed,
        stats.changed_cells
    );
    html.push_str("<div class=\"legend\"><span>Red = left value</span><span>Green = right value</span><span>Hover for full text in the desktop app.</span></div>");
    for warning in &compare.warnings {
        let _ = write!(html, "<div class=\"note\">{}</div>", escape_html(warning));
    }
    html.push_str("<div class=\"wrap\"><table><thead><tr><th class=\"rn\">Row</th>");
    for col in 0..compare.column_count {
        let changed = compare.changed_headers.contains(&col);
        let _ = write!(
            html,
            "<th class=\"{}\">{}</th>",
            if changed { "hdr-changed" } else { "" },
            escape_html(&compare.column_name(col))
        );
    }
    html.push_str("</tr></thead><tbody>");
    for row in &compare.rows {
        let class = match row.kind {
            CsvRowKind::Equal => "eq",
            CsvRowKind::Modified => "mod",
            CsvRowKind::LeftOnly => "del",
            CsvRowKind::RightOnly => "add",
        };
        let _ = write!(html, "<tr class=\"{class}\"><td class=\"rn\">");
        match (row.left, row.right) {
            (Some(left), Some(right)) => {
                let _ = write!(
                    html,
                    "L{}<br>R{}",
                    compare.left.row_number(left),
                    compare.right.row_number(right)
                );
            }
            (Some(left), None) => {
                let _ = write!(html, "L{}", compare.left.row_number(left));
            }
            (None, Some(right)) => {
                let _ = write!(html, "R{}", compare.right.row_number(right));
            }
            (None, None) => {}
        }
        html.push_str("</td>");
        for col in 0..compare.column_count {
            let left_value = row
                .left
                .map(|idx| compare.left.cell(idx, col))
                .unwrap_or("");
            let right_value = row
                .right
                .map(|idx| compare.right.cell(idx, col))
                .unwrap_or("");
            if row.kind == CsvRowKind::Modified && row.changed_cols.contains(&col) {
                let _ = write!(
                    html,
                    "<td><span class=\"changed\">{}</span> &rarr; <span class=\"changed-right\">{}</span></td>",
                    escape_html(left_value),
                    escape_html(right_value)
                );
            } else {
                let value = if row.right.is_some() {
                    right_value
                } else {
                    left_value
                };
                let _ = write!(html, "<td>{}</td>", escape_html(value));
            }
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></div></body></html>");
    html
}

pub fn text_html_report(view: &TextDiffView, left_name: &str, right_name: &str) -> String {
    let mut html = String::new();
    let stats = view.stats;
    html.push_str("<!doctype html><html><head><meta charset=\"utf-8\">");
    let _ = write!(
        html,
        "<title>Text comparison - {} vs {}</title>",
        escape_html(left_name),
        escape_html(right_name)
    );
    html.push_str(
        r#"<style>
:root{color-scheme:dark}body{font:13px/1.4 ui-sans-serif,system-ui,Segoe UI,sans-serif;background:#17191d;color:#dde1e8;margin:0;padding:24px}
h1{font-size:20px;margin:0 0 4px}.sub{color:#9aa4b2;margin-bottom:12px}
.stats{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}.chip{border:1px solid #353b45;border-radius:999px;padding:4px 10px;background:#22262d}
.wrap{overflow:auto;border:1px solid #303641;border-radius:8px;max-height:85vh}
table{border-collapse:separate;border-spacing:0;min-width:100%;font:12px/1.5 ui-monospace,Consolas,monospace;table-layout:fixed}
th{position:sticky;top:0;background:#22262d;padding:7px 10px;text-align:left;border-bottom:1px solid #3a414c}
td{padding:2px 8px;vertical-align:top;white-space:pre-wrap;overflow-wrap:anywhere}
.ln{width:54px;color:#76808e;text-align:right;user-select:none;background:#1b1e23}
.sep{width:1px;background:#303641;padding:0}
.lft{width:48%}.rgt{width:48%}
.mod .lft{background:#33291c}.mod .rgt{background:#1b2d24}.del .lft{background:#321f22}.add .rgt{background:#172820}
.hl{background:#7a3b36;color:#ffd7d2;border-radius:2px}.rgt .hl{background:#2e6a4b;color:#d7ffe7}
.s{background:#7a641c;color:#fff4c2;border-radius:2px}
.a{color:#ff9b9b}.b{color:#9fe3b5}.muted{color:#8f99a7}
</style></head><body>"#,
    );
    let _ = write!(
        html,
        "<h1>Text comparison</h1><div class=\"sub\"><span class=\"a\">{}</span> &harr; <span class=\"b\">{}</span></div>",
        escape_html(left_name),
        escape_html(right_name)
    );
    let _ = write!(
        html,
        "<div class=\"stats\"><span class=\"chip\">lines <b>{}</b> / <b>{}</b></span><span class=\"chip\">modified <b>{}</b></span><span class=\"chip\">added <b>{}</b></span><span class=\"chip\">removed <b>{}</b></span><span class=\"chip\">hunks <b>{}</b></span></div>",
        stats.left_lines, stats.right_lines, stats.modified, stats.added, stats.removed, stats.hunks
    );
    html.push_str(
        "<div class=\"wrap\"><table><thead><tr><th class=\"ln\">L</th><th class=\"lft\">",
    );
    let _ = write!(html, "{}", escape_html(left_name));
    html.push_str("</th><th class=\"ln\">R</th><th class=\"rgt\">");
    let _ = write!(html, "{}", escape_html(right_name));
    html.push_str("</th></tr></thead><tbody>");
    for row in &view.rows {
        let class = match row.kind {
            crate::textdiff::RowKind::Equal => "eq",
            crate::textdiff::RowKind::Modified => "mod",
            crate::textdiff::RowKind::LeftOnly => "del",
            crate::textdiff::RowKind::RightOnly => "add",
        };
        let _ = write!(html, "<tr class=\"{class}\">");
        let _ = write!(
            html,
            "<td class=\"ln\">{}</td>",
            row.left
                .map(|idx| idx + 1)
                .map(|n| n.to_string())
                .unwrap_or_default()
        );
        html.push_str("<td class=\"lft\">");
        if let Some(left) = row.left {
            html.push_str(&highlighted_html(
                view.left_line(left),
                &row.left_spans,
                "hl",
            ));
        }
        html.push_str("</td><td class=\"ln\">");
        if let Some(right) = row.right {
            let _ = write!(html, "{}", right + 1);
        }
        html.push_str("</td><td class=\"rgt\">");
        if let Some(right) = row.right {
            html.push_str(&highlighted_html(
                view.right_line(right),
                &row.right_spans,
                "hl",
            ));
        }
        html.push_str("</td></tr>");
    }
    html.push_str("</tbody></table></div></body></html>");
    html
}

fn highlighted_html(text: &str, spans: &[std::ops::Range<usize>], class: &str) -> String {
    let mut byte_spans = spans
        .iter()
        .filter_map(|span| char_range_to_bytes(text, span.clone()))
        .collect::<Vec<_>>();
    byte_spans.sort_by_key(|span| span.start);
    let mut out = String::new();
    let mut cursor = 0usize;
    for span in byte_spans {
        if span.end <= cursor || span.start > text.len() {
            continue;
        }
        let start = span.start.max(cursor);
        out.push_str(&escape_html(&text[cursor..start]));
        out.push_str("<span class=\"");
        out.push_str(class);
        out.push_str("\">");
        out.push_str(&escape_html(&text[start..span.end.min(text.len())]));
        out.push_str("</span>");
        cursor = span.end.min(text.len());
    }
    out.push_str(&escape_html(&text[cursor..]));
    out
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csvdiff::{compare_csv, CsvOptions};

    #[test]
    fn differences_csv_contains_changed_cell() {
        let compare = compare_csv(
            b"id,name\n1,alpha\n2,beta\n",
            b"id,name\n1,alpha\n2,BETA\n",
            &CsvOptions::default(),
        )
        .unwrap();
        let report = csv_differences(&compare);
        assert!(report.contains("modified"));
        assert!(report.contains("beta"));
        assert!(report.contains("BETA"));
    }

    #[test]
    fn html_report_escapes_values() {
        let compare = compare_csv(
            b"id,value\n1,<script>\n",
            b"id,value\n1,<script>\n",
            &CsvOptions::default(),
        )
        .unwrap();
        let report = csv_html_report(&compare, "a.csv", "b.csv");
        assert!(report.contains("&lt;script&gt;"));
        assert!(!report.contains("<script>"));
    }

    #[test]
    fn text_report_highlights_changes() {
        use crate::model::CompareOptions;
        use crate::textdiff::build_text_diff;
        let view = build_text_diff("hello\n", "hallo\n", &CompareOptions::default());
        let report = text_html_report(&view, "a.txt", "b.txt");
        assert!(report.contains("class=\"hl\""));
    }
}
