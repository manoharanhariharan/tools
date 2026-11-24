//! Server-side PDF generation. A4, clean table layout, repeated headers,
//! page numbers, and Indian money formatting. Uses an embedded system font when
//! available so `₹` renders; otherwise falls back to built-in Helvetica with
//! `Rs` prefixes.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use printpdf::*;
use ttf_parser::Face;

use crate::domain::date;
use crate::domain::money::{format_grouped, format_inr};
use crate::reports::summary::{MoneyTotals, ReportData, ReportLayout};

const ROW_HEIGHT_MM: f64 = 5.6;
const FIRST_ROW_OFFSET_MM: f64 = 41.0;
const BOTTOM_MARGIN_MM: f64 = 14.0;
const SIDE_MARGIN_MM: f64 = 13.0;

/// A TrueType font used for PDF text. The font is loaded once at startup and
/// kept in memory; only its metrics are consulted per report.
pub struct PdfFont {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub has_rupee_glyph: bool,
}

impl std::fmt::Debug for PdfFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfFont")
            .field("path", &self.path)
            .field("has_rupee_glyph", &self.has_rupee_glyph)
            .finish()
    }
}

impl PdfFont {
    /// Explicit path, then well-known platform fonts. Prefers a font that can
    /// draw U+20B9 (₹); otherwise falls back to `Rs` rendering.
    pub fn discover(explicit: Option<&Path>) -> anyhow::Result<Self> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(path) = explicit {
            candidates.push(path.to_path_buf());
        }
        if cfg!(windows) {
            candidates.push(PathBuf::from(r"C:\Windows\Fonts\segoeui.ttf"));
            candidates.push(PathBuf::from(r"C:\Windows\Fonts\arial.ttf"));
        } else if cfg!(target_os = "macos") {
            candidates.push(PathBuf::from(
                "/System/Library/Fonts/Supplemental/Arial.ttf",
            ));
        } else {
            candidates.push(PathBuf::from(
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ));
            candidates.push(PathBuf::from(
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            ));
            candidates.push(PathBuf::from(
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            ));
        }

        let mut fallback: Option<PdfFont> = None;
        for path in candidates {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(face) = Face::parse(&bytes, 0) else {
                continue;
            };
            let has_rupee_glyph = face.glyph_index('\u{20B9}').is_some();
            let font = PdfFont {
                path,
                bytes,
                has_rupee_glyph,
            };
            if has_rupee_glyph {
                return Ok(font);
            }
            if fallback.is_none() {
                fallback = Some(font);
            }
        }

        fallback.ok_or_else(|| {
            anyhow::anyhow!("no usable TrueType font found; set DAYBOOK_PDF_FONT to a .ttf file")
        })
    }

    /// Format a money amount for PDF output, respecting the font's glyph set.
    pub fn money(&self, minor: i64) -> String {
        if self.has_rupee_glyph {
            format_inr(minor)
        } else {
            format!("Rs {}", format_grouped(minor))
        }
    }

    pub fn units_per_em(&self) -> f64 {
        Face::parse(&self.bytes, 0)
            .map(|face| face.units_per_em() as f64)
            .unwrap_or(1_000.0)
    }

    /// Approximate advance width in points for a single line of text.
    /// Kerning/ligatures are intentionally ignored: table layout only needs
    /// stable right-aligned columns.
    pub fn width_pt(&self, text: &str, size_pt: f64) -> f64 {
        let Ok(face) = Face::parse(&self.bytes, 0) else {
            return text.chars().count() as f64 * size_pt * 0.5;
        };
        let upem = face.units_per_em() as f64;
        let units: f64 = text
            .chars()
            .map(|ch| {
                face.glyph_index(ch)
                    .and_then(|glyph| face.glyph_hor_advance(glyph))
                    .map(|advance| advance as f64)
                    .unwrap_or(upem * 0.5)
            })
            .sum();
        units / upem * size_pt
    }
}

/// Load a font from bytes for tests or callers that already have the file.
pub fn font_from_bytes(path: &Path, bytes: Vec<u8>) -> anyhow::Result<PdfFont> {
    let face = Face::parse(&bytes, 0).context("not a parseable TrueType font")?;
    if face.units_per_em() == 0 {
        bail!("font reports zero units per em");
    }
    let has_rupee_glyph = face.glyph_index('\u{20B9}').is_some();
    Ok(PdfFont {
        path: path.to_path_buf(),
        bytes,
        has_rupee_glyph,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Right,
}

struct Column {
    header: &'static str,
    width_pct: f64,
    align: Align,
}

struct RenderedTable {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    totals: Vec<String>,
    landscape: bool,
}

pub fn render(
    data: &ReportData,
    layout: ReportLayout,
    font: Option<&PdfFont>,
) -> anyhow::Result<Vec<u8>> {
    let table = build_table(data, layout, font);
    let external = font.is_some();
    let (page_w, page_h) = if table.landscape {
        (297.0, 210.0)
    } else {
        (210.0, 297.0)
    };

    let (doc, first_page, first_layer) = PdfDocument::new(
        "Daybook earnings report",
        Mm(page_w as f32),
        Mm(page_h as f32),
        "Report",
    );

    let font_ref = if let Some(font) = font {
        doc.add_external_font(Cursor::new(font.bytes.clone()))
            .context("cannot embed PDF font")?
    } else {
        doc.add_builtin_font(BuiltinFont::Helvetica)
            .context("cannot add built-in PDF font")?
    };

    let per_page = rows_per_page(page_h);
    let data_pages = if table.rows.is_empty() {
        1
    } else {
        table.rows.len().div_ceil(per_page)
    };
    let last_page_rows = if table.rows.is_empty() {
        0
    } else {
        table.rows.len() - (data_pages - 1) * per_page
    };
    let totals_on_last_data_page = table.rows.is_empty() || last_page_rows < per_page;
    let total_pages = data_pages + if totals_on_last_data_page { 0 } else { 1 };

    let mut page_number = 0usize;
    for page_index in 0..data_pages {
        page_number += 1;
        let (page, layer_index) = if page_index == 0 {
            (first_page, first_layer)
        } else {
            doc.add_page(Mm(page_w as f32), Mm(page_h as f32), "Report")
        };
        let layer = doc.get_page(page).get_layer(layer_index);

        let mut y = draw_header(
            &layer, data, &table, page_w, page_h, font, &font_ref, external,
        );

        let start = page_index * per_page;
        let end = (start + per_page).min(table.rows.len());
        if table.rows.is_empty() {
            layer.use_text(
                "No entries in this period.",
                9.0,
                Mm(nm(SIDE_MARGIN_MM)),
                Mm(nm(y - 6.0)),
                &font_ref,
            );
        } else {
            for row in &table.rows[start..end] {
                draw_row(
                    &layer,
                    &table.columns,
                    row,
                    y,
                    page_w,
                    font,
                    &font_ref,
                    external,
                    false,
                );
                y -= ROW_HEIGHT_MM;
            }
        }

        if page_index == data_pages - 1 && totals_on_last_data_page {
            draw_totals(&layer, &table, y, page_w, font, &font_ref, external);
        }
        draw_footer(&layer, page_w, page_number, total_pages, &font_ref);
    }

    if !totals_on_last_data_page {
        page_number += 1;
        let (page, layer_index) = doc.add_page(Mm(page_w as f32), Mm(page_h as f32), "Report");
        let layer = doc.get_page(page).get_layer(layer_index);
        let y = draw_header(
            &layer, data, &table, page_w, page_h, font, &font_ref, external,
        );
        draw_totals(&layer, &table, y, page_w, font, &font_ref, external);
        draw_footer(&layer, page_w, page_number, total_pages, &font_ref);
    }

    let bytes = doc.save_to_bytes().context("cannot serialise PDF")?;
    Ok(bytes)
}

fn nm(value: f64) -> f32 {
    value as f32
}

fn points_to_mm(points: f64) -> f64 {
    points * 25.4 / 72.0
}

fn rows_per_page(page_h: f64) -> usize {
    let usable = page_h - FIRST_ROW_OFFSET_MM - BOTTOM_MARGIN_MM;
    ((usable / ROW_HEIGHT_MM).floor() as usize).max(1)
}

fn money(font: Option<&PdfFont>, minor: i64) -> String {
    match font {
        Some(font) => font.money(minor),
        None => format!("Rs {}", format_grouped(minor)),
    }
}

fn build_table(data: &ReportData, layout: ReportLayout, font: Option<&PdfFont>) -> RenderedTable {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let totals = match layout {
        ReportLayout::Sunday => sunday_totals_row(data, font),
        ReportLayout::Entries => entries_totals_row(&data.totals, font),
        ReportLayout::Daily => daily_totals_row(&data.totals, font),
        ReportLayout::Employee => totals_row(&data.totals, font),
    };
    let landscape;
    let columns;

    match layout {
        ReportLayout::Sunday => {
            landscape = false;
            columns = vec![
                Column {
                    header: "Sl no",
                    width_pct: 8.0,
                    align: Align::Right,
                },
                Column {
                    header: "Name",
                    width_pct: 40.0,
                    align: Align::Left,
                },
                Column {
                    header: "Previous total",
                    width_pct: 17.0,
                    align: Align::Right,
                },
                Column {
                    header: "Sunday Earning",
                    width_pct: 17.0,
                    align: Align::Right,
                },
                Column {
                    header: "Current total",
                    width_pct: 18.0,
                    align: Align::Right,
                },
            ];
            for (index, row) in data.ledger.iter().enumerate() {
                rows.push(vec![
                    (index + 1).to_string(),
                    row.name.clone(),
                    money(font, row.previous_minor),
                    money(font, row.sunday_minor),
                    money(font, row.current_minor),
                ]);
            }
        }
        ReportLayout::Employee => {
            landscape = false;
            columns = vec![
                Column {
                    header: "Employee",
                    width_pct: 24.0,
                    align: Align::Left,
                },
                Column {
                    header: "Code",
                    width_pct: 9.0,
                    align: Align::Left,
                },
                Column {
                    header: "Sheets",
                    width_pct: 6.0,
                    align: Align::Right,
                },
                Column {
                    header: "Business",
                    width_pct: 13.0,
                    align: Align::Right,
                },
                Column {
                    header: "Bonus",
                    width_pct: 9.0,
                    align: Align::Right,
                },
                Column {
                    header: "Gross",
                    width_pct: 12.0,
                    align: Align::Right,
                },
                Column {
                    header: "Advance",
                    width_pct: 9.0,
                    align: Align::Right,
                },
                Column {
                    header: "Deduction",
                    width_pct: 9.0,
                    align: Align::Right,
                },
                Column {
                    header: "Final",
                    width_pct: 9.0,
                    align: Align::Right,
                },
            ];
            for employee in &data.employees {
                let t = &employee.totals;
                rows.push(vec![
                    employee.name.clone(),
                    employee.code.clone().unwrap_or_else(|| "—".to_string()),
                    t.entries.to_string(),
                    money(font, t.business_minor),
                    money(font, t.bonus_minor),
                    money(font, t.gross_minor),
                    money(font, t.advance_minor),
                    money(font, t.deduction_minor),
                    money(font, t.net_minor),
                ]);
            }
        }
        ReportLayout::Daily => {
            landscape = false;
            columns = vec![
                Column {
                    header: "Date",
                    width_pct: 20.0,
                    align: Align::Left,
                },
                Column {
                    header: "Sheets",
                    width_pct: 7.0,
                    align: Align::Right,
                },
                Column {
                    header: "Business",
                    width_pct: 15.0,
                    align: Align::Right,
                },
                Column {
                    header: "Bonus",
                    width_pct: 10.0,
                    align: Align::Right,
                },
                Column {
                    header: "Gross",
                    width_pct: 14.0,
                    align: Align::Right,
                },
                Column {
                    header: "Advance",
                    width_pct: 11.0,
                    align: Align::Right,
                },
                Column {
                    header: "Deduction",
                    width_pct: 11.0,
                    align: Align::Right,
                },
                Column {
                    header: "Final",
                    width_pct: 12.0,
                    align: Align::Right,
                },
            ];
            for day in &data.days {
                let t = &day.totals;
                rows.push(vec![
                    date::parse_iso(&day.work_date)
                        .map(date::display)
                        .unwrap_or_else(|| day.work_date.clone()),
                    t.entries.to_string(),
                    money(font, t.business_minor),
                    money(font, t.bonus_minor),
                    money(font, t.gross_minor),
                    money(font, t.advance_minor),
                    money(font, t.deduction_minor),
                    money(font, t.net_minor),
                ]);
            }
        }
        ReportLayout::Entries => {
            landscape = true;
            columns = vec![
                Column {
                    header: "Date",
                    width_pct: 11.0,
                    align: Align::Left,
                },
                Column {
                    header: "Employee",
                    width_pct: 23.0,
                    align: Align::Left,
                },
                Column {
                    header: "Business",
                    width_pct: 14.0,
                    align: Align::Right,
                },
                Column {
                    header: "Bonus",
                    width_pct: 9.0,
                    align: Align::Right,
                },
                Column {
                    header: "Gross",
                    width_pct: 12.0,
                    align: Align::Right,
                },
                Column {
                    header: "Advance",
                    width_pct: 9.0,
                    align: Align::Right,
                },
                Column {
                    header: "Deduction",
                    width_pct: 9.0,
                    align: Align::Right,
                },
                Column {
                    header: "Final",
                    width_pct: 13.0,
                    align: Align::Right,
                },
            ];
            for line in &data.lines {
                rows.push(vec![
                    date::parse_iso(&line.work_date)
                        .map(date::display)
                        .unwrap_or_else(|| line.work_date.clone()),
                    line.employee_name.clone(),
                    money(font, line.business_minor),
                    money(font, line.bonus_minor),
                    money(font, line.gross_minor),
                    money(font, line.advance_minor),
                    money(font, line.deduction_minor),
                    money(font, line.net_minor),
                ]);
            }
        }
    }

    let total_pct: f64 = columns.iter().map(|c| c.width_pct).sum();
    debug_assert!((total_pct - 100.0).abs() < 0.01, "columns must sum to 100%");

    RenderedTable {
        columns,
        rows,
        totals,
        landscape,
    }
}

fn totals_row(totals: &MoneyTotals, font: Option<&PdfFont>) -> Vec<String> {
    vec![
        "TOTAL".to_string(),
        String::new(),
        totals.entries.to_string(),
        money(font, totals.business_minor),
        money(font, totals.bonus_minor),
        money(font, totals.gross_minor),
        money(font, totals.advance_minor),
        money(font, totals.deduction_minor),
        money(font, totals.net_minor),
    ]
}

fn sunday_totals_row(data: &ReportData, font: Option<&PdfFont>) -> Vec<String> {
    vec![
        String::new(),
        "TOTAL".to_string(),
        money(font, data.previous_total_minor),
        money(font, data.sunday_total_minor),
        money(font, data.current_total_minor),
    ]
}

fn daily_totals_row(totals: &MoneyTotals, font: Option<&PdfFont>) -> Vec<String> {
    vec![
        "TOTAL".to_string(),
        totals.entries.to_string(),
        money(font, totals.business_minor),
        money(font, totals.bonus_minor),
        money(font, totals.gross_minor),
        money(font, totals.advance_minor),
        money(font, totals.deduction_minor),
        money(font, totals.net_minor),
    ]
}

/// Detail layout columns are Date, Employee, Business, Bonus, Gross, Advance,
/// Deduction, Final — no sheet count.
fn entries_totals_row(totals: &MoneyTotals, font: Option<&PdfFont>) -> Vec<String> {
    vec![
        "TOTAL".to_string(),
        String::new(),
        money(font, totals.business_minor),
        money(font, totals.bonus_minor),
        money(font, totals.gross_minor),
        money(font, totals.advance_minor),
        money(font, totals.deduction_minor),
        money(font, totals.net_minor),
    ]
}

#[allow(clippy::too_many_arguments)]
fn draw_header(
    layer: &PdfLayerReference,
    data: &ReportData,
    table: &RenderedTable,
    page_w: f64,
    page_h: f64,
    font: Option<&PdfFont>,
    font_ref: &IndirectFontRef,
    external: bool,
) -> f64 {
    let left = SIDE_MARGIN_MM;

    layer.use_text(
        "Daybook — employee earnings",
        15.0,
        Mm(nm(left)),
        Mm(nm(page_h - 15.0)),
        font_ref,
    );

    let period = format!(
        "{} to {}  ·  {} entries  ·  generated {} IST",
        date::parse_iso(&data.from)
            .map(date::display)
            .unwrap_or_else(|| data.from.clone()),
        date::parse_iso(&data.to)
            .map(date::display)
            .unwrap_or_else(|| data.to.clone()),
        data.totals.entries,
        date::format_datetime_ist(data.generated_at)
    );
    set_grey(layer, 0.35);
    layer.use_text(
        ascii_if_needed(&period, external),
        8.5,
        Mm(nm(left)),
        Mm(nm(page_h - 21.5)),
        font_ref,
    );

    let summary = format!(
        "Business {}   Bonus {}   Advances {}   Deductions {}   Final money {}",
        money(font, data.totals.business_minor),
        money(font, data.totals.bonus_minor),
        money(font, data.totals.advance_minor),
        money(font, data.totals.deduction_minor),
        money(font, data.totals.net_minor)
    );
    set_black(layer);
    layer.use_text(
        ascii_if_needed(&summary, external),
        9.0,
        Mm(nm(left)),
        Mm(nm(page_h - 28.5)),
        font_ref,
    );

    // Column headers
    set_grey(layer, 0.35);
    let mut x = left;
    for column in &table.columns {
        let width_mm = (page_w - 2.0 * SIDE_MARGIN_MM) * column.width_pct / 100.0;
        draw_text(
            layer,
            column.header,
            7.5,
            x,
            page_h - 35.0,
            column.align,
            width_mm,
            font,
            font_ref,
            external,
        );
        x += width_mm;
    }
    draw_rule(layer, left, page_w - SIDE_MARGIN_MM, page_h - 36.5, 0.6);
    set_black(layer);

    page_h - FIRST_ROW_OFFSET_MM
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    layer: &PdfLayerReference,
    columns: &[Column],
    row: &[String],
    baseline: f64,
    page_w: f64,
    font: Option<&PdfFont>,
    font_ref: &IndirectFontRef,
    external: bool,
    bold: bool,
) {
    let mut x = SIDE_MARGIN_MM;
    for (column, value) in columns.iter().zip(row.iter()) {
        let width_mm = (page_w - 2.0 * SIDE_MARGIN_MM) * column.width_pct / 100.0;
        let size = if bold { 8.5 } else { 8.0 };
        draw_text(
            layer,
            value,
            size,
            x,
            baseline,
            column.align,
            width_mm - 2.0,
            font,
            font_ref,
            external,
        );
        x += width_mm;
    }
}

fn draw_totals(
    layer: &PdfLayerReference,
    table: &RenderedTable,
    baseline: f64,
    page_w: f64,
    font: Option<&PdfFont>,
    font_ref: &IndirectFontRef,
    external: bool,
) {
    let baseline = baseline.max(BOTTOM_MARGIN_MM + 2.0);
    draw_rule(
        layer,
        SIDE_MARGIN_MM,
        page_w - SIDE_MARGIN_MM,
        baseline + 4.0,
        0.8,
    );
    draw_row(
        layer,
        &table.columns,
        &table.totals,
        baseline,
        page_w,
        font,
        font_ref,
        external,
        true,
    );
}

fn draw_footer(
    layer: &PdfLayerReference,
    page_w: f64,
    page: usize,
    total: usize,
    font_ref: &IndirectFontRef,
) {
    let label = format!("Daybook  ·  page {page} of {total}  ·  amounts in INR");
    set_grey(layer, 0.45);
    let approx_width = label.chars().count() as f64 * 4.2;
    layer.use_text(
        &label,
        7.5,
        Mm(nm((page_w - approx_width) / 2.0)),
        Mm(8.0),
        font_ref,
    );
    set_black(layer);
}

fn draw_rule(layer: &PdfLayerReference, x1: f64, x2: f64, y: f64, thickness: f32) {
    set_grey(layer, 0.75);
    layer.set_outline_thickness(thickness);
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(nm(x1)), Mm(nm(y))), false),
            (Point::new(Mm(nm(x2)), Mm(nm(y))), false),
        ],
        is_closed: false,
    });
    set_black(layer);
}

fn set_grey(layer: &PdfLayerReference, value: f32) {
    layer.set_fill_color(Color::Greyscale(Greyscale::new(value, None)));
    layer.set_outline_color(Color::Greyscale(Greyscale::new(value, None)));
}

fn set_black(layer: &PdfLayerReference) {
    layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
    layer.set_outline_color(Color::Greyscale(Greyscale::new(0.0, None)));
}

fn ascii_if_needed(text: &str, external: bool) -> String {
    if external {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '₹' => out.push_str("Rs "),
            '—' | '·' | '–' => out.push('-'),
            c if c.is_ascii() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

fn text_width_pt(text: &str, size_pt: f64, font: Option<&PdfFont>) -> f64 {
    match font {
        Some(font) => font.width_pt(text, size_pt),
        None => text.chars().count() as f64 * size_pt * 0.5,
    }
}

fn fit_text(
    text: &str,
    max_width_mm: f64,
    size_pt: f64,
    font: Option<&PdfFont>,
    external: bool,
) -> String {
    let display = ascii_if_needed(text, external);
    let max_pt = max_width_mm * 72.0 / 25.4;
    if text_width_pt(&display, size_pt, font) <= max_pt {
        return display;
    }
    let suffix = if external { "…" } else { "..." };
    let mut kept = String::new();
    for ch in display.chars() {
        let mut candidate = kept.clone();
        candidate.push(ch);
        candidate.push_str(suffix);
        if text_width_pt(&candidate, size_pt, font) > max_pt {
            break;
        }
        kept.push(ch);
    }
    kept.push_str(suffix);
    kept
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    layer: &PdfLayerReference,
    text: &str,
    size_pt: f64,
    x_mm: f64,
    baseline_mm: f64,
    align: Align,
    max_width_mm: f64,
    font: Option<&PdfFont>,
    font_ref: &IndirectFontRef,
    external: bool,
) {
    let fitted = fit_text(text, max_width_mm, size_pt, font, external);
    let width_mm = points_to_mm(text_width_pt(&fitted, size_pt, font));
    let x = match align {
        Align::Left => x_mm,
        Align::Right => x_mm + max_width_mm - width_mm,
    };
    layer.use_text(
        &fitted,
        size_pt as f32,
        Mm(nm(x.max(0.0))),
        Mm(nm(baseline_mm)),
        font_ref,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::entries::{EntryLineRow, EntryRow};

    fn sample(lines: usize) -> ReportData {
        let lines: Vec<EntryRow> = (0..lines)
            .map(|index| EntryRow {
                id: format!("id-{index}"),
                employee_id: format!("e{}", index % 3),
                work_date: format!("2026-11-{:02}", (index % 28) + 1),
                business_minor: 80_000,
                bonus_minor: 0,
                advance_minor: 5_000,
                deduction_minor: 0,
                gross_minor: 80_000,
                net_minor: 75_000,
                note: String::new(),
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                seq: index as i64 + 1,
                employee_name: format!("Employee {}", index % 3),
                employee_code: None,
                lines: vec![EntryLineRow {
                    id: format!("line-{index}"),
                    entry_id: format!("id-{index}"),
                    type_id: "sales".into(),
                    type_name: "Sales".into(),
                    description: String::new(),
                    amount_minor: 80_000,
                    position: 0,
                    created_at: 0,
                }],
            })
            .collect();
        crate::reports::summary::build("2026-11-01", "2026-11-30", 1_700_000_000_000, lines)
    }

    fn pdf_for(data: &ReportData, layout: ReportLayout) -> Vec<u8> {
        match PdfFont::discover(None) {
            Ok(font) => render(data, layout, Some(&font)).expect("render with font"),
            Err(_) => render(data, layout, None).expect("render with builtin font"),
        }
    }

    #[test]
    fn produces_a_valid_pdf_document() {
        let bytes = pdf_for(&sample(3), ReportLayout::Employee);
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 2_000);
        let tail = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(64)..]);
        assert!(tail.contains("%%EOF"), "missing EOF marker: {tail:?}");
    }

    #[test]
    fn handles_empty_period_and_all_layouts() {
        for layout in [
            ReportLayout::Sunday,
            ReportLayout::Employee,
            ReportLayout::Daily,
            ReportLayout::Entries,
        ] {
            let bytes = pdf_for(&sample(0), layout);
            assert!(bytes.starts_with(b"%PDF-"));
        }
        let detail = pdf_for(&sample(2), ReportLayout::Entries);
        assert!(detail.starts_with(b"%PDF-"));
        assert!(detail.len() > 2_000);
    }

    #[test]
    fn pagination_splits_and_keeps_all_rows() {
        assert!(rows_per_page(210.0) > 10);
        let per_page = rows_per_page(297.0);
        let data = sample(per_page * 2 + 1);
        let table = build_table(&data, ReportLayout::Entries, None);
        assert_eq!(table.rows.len(), per_page * 2 + 1);
        assert!(table.landscape);
        let employee_layout = build_table(&sample(1), ReportLayout::Employee, None);
        assert!(!employee_layout.landscape);
    }

    #[test]
    fn every_table_row_has_one_cell_per_column() {
        for layout in [
            ReportLayout::Sunday,
            ReportLayout::Employee,
            ReportLayout::Daily,
            ReportLayout::Entries,
        ] {
            let table = build_table(&sample(3), layout, None);
            assert_eq!(
                table.totals.len(),
                table.columns.len(),
                "totals row misaligned for {layout:?}"
            );
            for row in &table.rows {
                assert_eq!(
                    row.len(),
                    table.columns.len(),
                    "row misaligned for {layout:?}"
                );
            }
        }
    }

    #[test]
    fn long_text_is_truncated_to_its_column() {
        let long = "A very long employee name that cannot possibly fit in one column";
        let fitted = fit_text(long, 30.0, 8.0, None, false);
        assert!(fitted.len() < long.len());
        assert!(fitted.ends_with("..."));
        assert_eq!(fit_text("Asha", 40.0, 8.0, None, false), "Asha");
    }

    #[test]
    fn builtin_fallback_replaces_non_ascii() {
        assert_eq!(ascii_if_needed("₹1,234 — Asha", false), "Rs 1,234 - Asha");
        assert_eq!(ascii_if_needed("₹1,234", true), "₹1,234");
    }

    #[test]
    fn width_scales_with_size_and_content() {
        let Ok(font) = PdfFont::discover(None) else {
            return;
        };
        let small = font.width_pt("Asha", 9.0);
        let large = font.width_pt("Asha", 18.0);
        assert!(small > 0.0);
        assert!((large - small * 2.0).abs() < 0.5);
        assert!(font.width_pt("Asha", 9.0) < font.width_pt("Asha Kumari", 9.0));
    }

    #[test]
    fn money_formatting_respects_glyph_support() {
        let Ok(bytes) = std::fs::read(if cfg!(windows) {
            r"C:\Windows\Fonts\segoeui.ttf"
        } else {
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
        }) else {
            return;
        };
        let font = font_from_bytes(Path::new("test.ttf"), bytes).unwrap();
        if font.has_rupee_glyph {
            assert!(font.money(123_456).starts_with('₹'));
        } else {
            assert!(font.money(123_456).starts_with("Rs "));
        }
    }
}
