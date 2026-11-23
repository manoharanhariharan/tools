//! CSV table rendering, side-by-side panes and cell painting.

use super::menus::*;
use super::*;

pub(super) struct CsvTableContext<'a> {
    pub compare: &'a CsvCompare,
    pub visible: &'a [usize],
    pub columns: &'a [usize],
    pub widths: &'a [f32],
    pub selected_row: Option<usize>,
    pub selected_cell: Option<(usize, usize)>,
    pub scroll_row: Option<usize>,
    pub focus_col: Option<usize>,
    pub query: &'a str,
    pub case_sensitive: bool,
}

struct PaneLayout {
    gutter: f32,
    content_width: f32,
    sync: egui::Vec2,
    left_side: bool,
}

struct MergedCell<'a> {
    compare: &'a CsvCompare,
    row: usize,
    col: usize,
    selected: bool,
    selected_cell: bool,
    query: &'a str,
    case_sensitive: bool,
}

struct SideCell<'a> {
    compare: &'a CsvCompare,
    row: usize,
    col: usize,
    value: &'a str,
    present: bool,
    kind: CsvRowKind,
    left_side: bool,
    changed: bool,
    spans: &'a [std::ops::Range<usize>],
    selected: bool,
    selected_cell: bool,
    query: &'a str,
    case_sensitive: bool,
    tooltip: &'a str,
}

pub(super) fn merged_table(ui: &mut Ui, ctx: &CsvTableContext<'_>, state: &mut CsvUiState) {
    let compare = ctx.compare;
    let visible = ctx.visible;
    let columns = ctx.columns;
    let widths = ctx.widths;
    let selected_row = ctx.selected_row;
    let selected_cell = ctx.selected_cell;
    let scroll_row = ctx.scroll_row;
    let focus_col = ctx.focus_col;
    let query = ctx.query;
    let case_sensitive = ctx.case_sensitive;
    let gutter = 82.0;
    let total = gutter
        + columns.iter().map(|col| widths[*col]).sum::<f32>()
        + columns.len() as f32 * 10.0
        + 30.0;
    let viewport = ui.available_width().max(160.0);
    let pane_height = (ui.available_height() - 24.0).max(80.0);
    let mut initial_x = state.merged_scroll_x;
    if let Some(col) = focus_col {
        if let Some(pos) = columns.iter().position(|candidate| *candidate == col) {
            let before: f32 = columns[..pos]
                .iter()
                .map(|candidate| widths[*candidate] + 10.0)
                .sum();
            initial_x = (gutter + before + widths[col] / 2.0 - viewport / 2.0).max(0.0);
            state.merged_scroll_x = initial_x;
        }
    }
    let area = egui::ScrollArea::horizontal()
        .id_salt("csv_merged_h")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, false])
        .horizontal_scroll_offset(initial_x);
    let pane = ui.allocate_ui(egui::vec2(ui.available_width(), pane_height), |ui| {
        area.show(ui, |ui| {
            ui.set_width(total);
            let mut builder = TableBuilder::new(ui)
                .id_salt("csv_merged")
                .striped(false)
                .resizable(false)
                .sense(Sense::click())
                .column(Column::exact(gutter).clip(true));
            for col in columns {
                builder = builder.column(Column::exact(widths[*col]).clip(true));
            }
            if let Some(row) = scroll_row {
                builder = builder.scroll_to_row(row, Some(Align::Center));
            }
            let table = builder.header(30.0, |mut header| {
                header.col(|ui| {
                    ui.label(
                        RichText::new("ROW")
                            .color(theme::muted())
                            .size(10.5)
                            .strong(),
                    );
                });
                for col in columns {
                    let col = *col;
                    header.col(|ui| {
                        let changed = compare.changed_headers.contains(&col);
                        paint_column_header(ui, compare, col);
                        let label = Label::new(
                            RichText::new(compare.column_name(col))
                                .color(if changed {
                                    theme::diff()
                                } else {
                                    theme::text()
                                })
                                .size(11.5)
                                .strong(),
                        )
                        .truncate()
                        .sense(Sense::click());
                        ui.add(label);
                        let response = ui.interact(
                            ui.max_rect(),
                            ui.id().with(("csv_merged_header", col)),
                            Sense::click(),
                        );
                        response.context_menu(|ui| {
                            column_context_menu(ui, compare, col, state);
                        });
                        if let Some(tip) = column_header_tooltip(compare, col) {
                            response.on_hover_ui(|ui| {
                                ui.set_max_width(420.0);
                                ui.label(RichText::new(tip).monospace().size(12.0));
                            });
                        }
                    });
                }
            });
            table.body(|body| {
                let heights = visible.iter().map(|idx| {
                    if compare.rows[*idx].kind == CsvRowKind::Modified {
                        42.0
                    } else {
                        25.0
                    }
                });
                body.heterogeneous_rows(heights, |mut row| {
                    let table_idx = row.index();
                    let idx = visible[table_idx];
                    let selected = selected_row == Some(idx);
                    row.set_selected(selected);
                    let (_, gutter_response) = row.col(|ui| {
                        paint_row_gutter(ui, compare, idx, selected);
                    });
                    gutter_response.context_menu(|ui| {
                        row_context_menu(ui, compare, idx);
                    });
                    for col in columns {
                        let col = *col;
                        row.col(|ui| {
                            let response = paint_merged_cell(
                                ui,
                                &MergedCell {
                                    compare,
                                    row: idx,
                                    col,
                                    selected,
                                    selected_cell: selected_cell == Some((idx, col)),
                                    query,
                                    case_sensitive,
                                },
                                state,
                            );
                            if response.clicked() {
                                state.selected_row = Some(idx);
                                state.selected_cell = Some((idx, col));
                            }
                        });
                    }
                    if row.response().clicked() {
                        state.selected_row = Some(idx);
                    }
                });
            });
        })
    });
    state.merged_scroll_x = pane.inner.state.offset.x;
    if let Some(offset) = common::horizontal_scroll_bar(ui, state.merged_scroll_x, viewport, total)
    {
        state.merged_scroll_x = offset;
    }
}

pub(super) fn side_by_side_table(ui: &mut Ui, ctx: &CsvTableContext<'_>, state: &mut CsvUiState) {
    let columns = ctx.columns;
    let widths = ctx.widths;
    let scroll_row = ctx.scroll_row;
    let focus_col = ctx.focus_col;
    let full_rect = ui.available_rect_before_wrap();
    let gutter = 54.0;
    let content_width = gutter
        + columns.iter().map(|col| widths[*col]).sum::<f32>()
        + columns.len() as f32 * 10.0
        + 20.0;
    let pane_height = (full_rect.height() - 24.0).max(80.0);

    let mut left_rect = egui::Rect::NOTHING;
    let mut right_rect = egui::Rect::NOTHING;
    if let Some(col) = focus_col {
        if let Some(pos) = columns.iter().position(|candidate| *candidate == col) {
            let before: f32 = columns[..pos]
                .iter()
                .map(|candidate| widths[*candidate] + 10.0)
                .sum();
            let viewport = (full_rect.width() * 0.5).max(140.0);
            state.sync_scroll_x = (gutter + before + widths[col] / 2.0 - viewport / 2.0).max(0.0);
        }
    }
    if let Some(row) = scroll_row {
        let viewport = (pane_height - 30.0).max(40.0);
        state.sync_scroll_y =
            common::row_scroll_target(row, 23.0, ui.spacing().item_spacing.y, viewport);
    }
    let mut left_offset = egui::Vec2::new(state.sync_scroll_x, state.sync_scroll_y);
    let mut right_offset = egui::Vec2::new(state.sync_scroll_x, state.sync_scroll_y);
    let sync = egui::Vec2::new(state.sync_scroll_x, state.sync_scroll_y);

    let pane = ui.allocate_ui(egui::vec2(full_rect.width(), pane_height), |ui| {
        ui.columns(2, |pane| {
            left_rect = pane[0].available_rect_before_wrap();
            left_offset = render_csv_pane(
                &mut pane[0],
                ctx,
                &PaneLayout {
                    gutter,
                    content_width,
                    sync,
                    left_side: true,
                },
                state,
            );
            right_rect = pane[1].available_rect_before_wrap();
            right_offset = render_csv_pane(
                &mut pane[1],
                ctx,
                &PaneLayout {
                    gutter,
                    content_width,
                    sync,
                    left_side: false,
                },
                state,
            );
        });
    });
    let pane_rect = pane.response.rect;

    common::sync_panes(
        &mut state.sync_scroll_x,
        &mut state.sync_scroll_y,
        left_rect,
        right_rect,
        left_offset,
        right_offset,
        ui.ctx().pointer_hover_pos(),
    );

    // The splitter lives outside both scroll areas, so it always stays in the
    // center of the viewport, even when either pane scrolls horizontally.
    let painter = ui.painter();
    painter.vline(
        pane_rect.center().x,
        pane_rect.y_range(),
        Stroke::new(1.0_f32, theme::border_light()),
    );
    for offset in [-5.0_f32, 0.0, 5.0] {
        painter.circle_filled(
            egui::pos2(pane_rect.center().x, pane_rect.top() + 15.0 + offset),
            1.5,
            theme::muted(),
        );
    }

    // One shared horizontal scrollbar keeps both panes aligned.
    let bar_offset = common::horizontal_scroll_bar(
        ui,
        state.sync_scroll_x,
        pane_rect.width() * 0.5,
        content_width,
    );
    if let Some(offset) = bar_offset {
        state.sync_scroll_x = offset;
    }
}

fn render_csv_pane(
    ui: &mut Ui,
    ctx: &CsvTableContext<'_>,
    pane: &PaneLayout,
    state: &mut CsvUiState,
) -> egui::Vec2 {
    let compare = ctx.compare;
    let visible = ctx.visible;
    let columns = ctx.columns;
    let widths = ctx.widths;
    let selected_row = ctx.selected_row;
    let selected_cell = ctx.selected_cell;
    let scroll_row = ctx.scroll_row;
    let focus_col = ctx.focus_col;
    let query = ctx.query;
    let case_sensitive = ctx.case_sensitive;
    let gutter = pane.gutter;
    let content_width = pane.content_width;
    let sync = pane.sync;
    let left_side = pane.left_side;
    let mut offset = sync;
    let id = if left_side {
        "csv_sbs_left_h"
    } else {
        "csv_sbs_right_h"
    };
    let mut initial_x = sync.x;
    if let Some(col) = focus_col {
        if let Some(pos) = columns.iter().position(|candidate| *candidate == col) {
            let before: f32 = columns[..pos]
                .iter()
                .map(|candidate| widths[*candidate] + 10.0)
                .sum();
            let viewport = ui.available_width().max(140.0);
            initial_x = (gutter + before + widths[col] / 2.0 - viewport / 2.0).max(0.0);
        }
    }
    let area_output = egui::ScrollArea::horizontal()
        .id_salt(id)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .horizontal_scroll_offset(initial_x)
        .show(ui, |ui| {
            ui.set_width(content_width);
            let mut builder = TableBuilder::new(ui)
                .id_salt(if left_side {
                    "csv_sbs_left"
                } else {
                    "csv_sbs_right"
                })
                .striped(false)
                .resizable(false)
                .sense(Sense::click())
                .vertical_scroll_offset(sync.y)
                .column(Column::exact(gutter).clip(true));
            for col in columns {
                builder = builder.column(Column::exact(widths[*col]).clip(true));
            }
            if let Some(row) = scroll_row {
                builder = builder.scroll_to_row(row, Some(Align::Center));
            }
            let table = builder.header(30.0, |mut header| {
                header.col(|ui| {
                    ui.label(
                        RichText::new(if left_side { "L" } else { "R" })
                            .color(if left_side {
                                theme::left()
                            } else {
                                theme::right()
                            })
                            .size(11.0)
                            .strong(),
                    );
                });
                for col in columns {
                    let col = *col;
                    header.col(|ui| {
                        let changed = compare.changed_headers.contains(&col);
                        paint_column_header(ui, compare, col);
                        ui.add(
                            Label::new(
                                RichText::new(if left_side {
                                    compare.left.display_header(col)
                                } else {
                                    compare.right.display_header(col)
                                })
                                .color(if changed {
                                    theme::diff()
                                } else {
                                    theme::text()
                                })
                                .size(11.0)
                                .strong(),
                            )
                            .truncate()
                            .sense(Sense::click()),
                        );
                        let response = ui.interact(
                            ui.max_rect(),
                            ui.id().with((
                                if left_side {
                                    "csv_sbs_left_header"
                                } else {
                                    "csv_sbs_right_header"
                                },
                                col,
                            )),
                            Sense::click(),
                        );
                        response.context_menu(|ui| {
                            column_context_menu(ui, compare, col, state);
                        });
                        if let Some(tip) = column_header_tooltip(compare, col) {
                            response.on_hover_ui(|ui| {
                                ui.set_max_width(420.0);
                                ui.label(RichText::new(tip).monospace().size(12.0));
                            });
                        }
                    });
                }
            });
            let output = table.body(|body| {
                body.rows(23.0, visible.len(), |mut row| {
                    let table_idx = row.index();
                    let idx = visible[table_idx];
                    let selected = selected_row == Some(idx);
                    row.set_selected(selected);
                    let (_, gutter_response) = row.col(|ui| {
                        paint_pane_gutter(ui, compare, idx, left_side, selected);
                    });
                    gutter_response.context_menu(|ui| {
                        row_context_menu(ui, compare, idx);
                    });
                    for col in columns {
                        let col = *col;
                        row.col(|ui| {
                            let value = if left_side {
                                compare.rows[idx]
                                    .left
                                    .map(|line| compare.left.cell(line, col))
                                    .unwrap_or("")
                            } else {
                                compare.rows[idx]
                                    .right
                                    .map(|line| compare.right.cell(line, col))
                                    .unwrap_or("")
                            };
                            let present = if left_side {
                                compare.rows[idx].left.is_some()
                            } else {
                                compare.rows[idx].right.is_some()
                            };
                            let changed = present
                                && (compare.rows[idx].changed_cols.contains(&col)
                                    || (!left_side
                                        && compare.rows[idx].kind == CsvRowKind::RightOnly)
                                    || (left_side
                                        && compare.rows[idx].kind == CsvRowKind::LeftOnly));
                            let spans = compare
                                .cell_diff(idx, col)
                                .map(|cell| {
                                    if left_side {
                                        cell.left_spans.as_slice()
                                    } else {
                                        cell.right_spans.as_slice()
                                    }
                                })
                                .unwrap_or(&[]);
                            let tooltip = if left_side {
                                format!("Left: {value}")
                            } else {
                                format!("Right: {value}")
                            };
                            let response = paint_side_cell(
                                ui,
                                &SideCell {
                                    compare,
                                    row: idx,
                                    col,
                                    value,
                                    present,
                                    kind: compare.rows[idx].kind,
                                    left_side,
                                    changed,
                                    spans,
                                    selected,
                                    selected_cell: selected_cell == Some((idx, col)),
                                    query,
                                    case_sensitive,
                                    tooltip: &tooltip,
                                },
                                state,
                            );
                            if response.clicked() {
                                state.selected_row = Some(idx);
                                state.selected_cell = Some((idx, col));
                            }
                        });
                    }
                    if row.response().clicked() {
                        state.selected_row = Some(idx);
                    }
                })
            });
            offset = output.state.offset;
        });
    offset.x = area_output.state.offset.x;
    offset
}

fn paint_pane_gutter(
    ui: &mut Ui,
    compare: &CsvCompare,
    row: usize,
    left_side: bool,
    selected: bool,
) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(
        rect,
        0.0,
        if selected {
            theme::sel_bg()
        } else {
            theme::gutter()
        },
    );
    let diff_row = &compare.rows[row];
    let (glyph, color) = status_glyph(diff_row.kind);
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        glyph,
        FontId::monospace(11.5),
        color,
    );
    let number = if left_side {
        diff_row
            .left
            .map(|line| compare.left.row_number(line).to_string())
    } else {
        diff_row
            .right
            .map(|line| compare.right.row_number(line).to_string())
    };
    ui.painter().text(
        egui::pos2(rect.right() - 7.0, rect.center().y),
        Align2::RIGHT_CENTER,
        number.unwrap_or_default(),
        FontId::monospace(10.5),
        if selected {
            theme::text()
        } else {
            theme::muted()
        },
    );
}

fn paint_row_gutter(ui: &mut Ui, compare: &CsvCompare, row: usize, selected: bool) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(
        rect,
        0.0,
        if selected {
            theme::sel_bg()
        } else {
            theme::gutter()
        },
    );
    let diff_row = &compare.rows[row];
    let (glyph, color) = status_glyph(diff_row.kind);
    ui.painter().text(
        egui::pos2(rect.left() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        glyph,
        FontId::monospace(12.0),
        color,
    );
    let numbers = match (diff_row.left, diff_row.right) {
        (Some(left), Some(right)) => format!(
            "{}·{}",
            compare.left.row_number(left),
            compare.right.row_number(right)
        ),
        (Some(left), None) => format!("−{}", compare.left.row_number(left)),
        (None, Some(right)) => format!("+{}", compare.right.row_number(right)),
        (None, None) => String::new(),
    };
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        numbers,
        FontId::monospace(10.5),
        if selected {
            theme::text()
        } else {
            theme::muted()
        },
    );
}

fn status_glyph(kind: CsvRowKind) -> (&'static str, Color32) {
    match kind {
        CsvRowKind::Equal => ("=", theme::faint()),
        CsvRowKind::Modified => ("~", theme::diff()),
        CsvRowKind::LeftOnly => ("-", theme::left()),
        CsvRowKind::RightOnly => ("+", theme::right()),
    }
}

/// Tint the header of a column that contains differing cells and draw a short
/// underline so the column stands out while scrolling the table.
fn paint_column_header(ui: &Ui, compare: &CsvCompare, col: usize) {
    if compare.column_change_count(col) == 0 {
        return;
    }
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 3.0, theme::mod_bg());
    let underline = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 5.0, rect.bottom() - 3.0),
        egui::pos2(rect.right() - 5.0, rect.bottom() - 1.0),
    );
    ui.painter().rect_filled(underline, 1.0, theme::diff());
}

fn paint_merged_cell(ui: &mut Ui, cell: &MergedCell<'_>, state: &mut CsvUiState) -> egui::Response {
    let compare = cell.compare;
    let row = cell.row;
    let col = cell.col;
    let selected = cell.selected;
    let selected_cell = cell.selected_cell;
    let query = cell.query;
    let case_sensitive = cell.case_sensitive;
    let diff_row = &compare.rows[row];
    let rect = ui.max_rect();
    let one_sided = matches!(diff_row.kind, CsvRowKind::LeftOnly | CsvRowKind::RightOnly);
    let mut bg = if selected || selected_cell {
        theme::sel_bg()
    } else {
        theme::cell_bg(diff_row.kind, diff_row.left.is_some())
    };
    if one_sided {
        bg = theme::cell_bg(diff_row.kind, diff_row.left.is_some());
    }
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    if selected_cell {
        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0_f32, theme::accent()),
            egui::StrokeKind::Inside,
        );
    }

    let cell = compare.cell_diff(row, col);
    let (left, right) = match (diff_row.left, diff_row.right) {
        (Some(left), Some(right)) => (
            Some(compare.left.cell(left, col)),
            Some(compare.right.cell(right, col)),
        ),
        (Some(left), None) => (Some(compare.left.cell(left, col)), None),
        (None, Some(right)) => (None, Some(compare.right.cell(right, col))),
        (None, None) => (None, None),
    };
    let display_color = theme::text();

    if let Some(cell) = cell {
        let left_display = display_value(if cell.left.is_empty() {
            "∅"
        } else {
            &cell.left
        });
        let right_display = display_value(if cell.right.is_empty() {
            "∅"
        } else {
            &cell.right
        });
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add(
                Label::new(colored_job(
                    &left_display,
                    theme::del_bg(),
                    display_color,
                    query,
                    case_sensitive,
                ))
                .wrap_mode(TextWrapMode::Extend)
                .selectable(false),
            );
            ui.add(
                Label::new(colored_job(
                    &right_display,
                    theme::add_bg(),
                    display_color,
                    query,
                    case_sensitive,
                ))
                .wrap_mode(TextWrapMode::Extend)
                .selectable(false),
            );
        });
    } else if let Some(value) = right.or(left) {
        let display = display_value(value);
        ui.add(
            Label::new(colored_job(
                &display,
                Color32::TRANSPARENT,
                display_color,
                query,
                case_sensitive,
            ))
            .wrap_mode(TextWrapMode::Extend)
            .selectable(false),
        );
    }

    let response = ui.interact(
        rect,
        ui.id().with(("csv_merged_cell", row, col)),
        Sense::click(),
    );
    let tooltip = match (cell, left, right) {
        (Some(cell), _, _) => format!("Left: {}\nRight: {}", cell.left, cell.right),
        (None, Some(left), Some(right)) if left == right => left.to_string(),
        (None, Some(left), Some(right)) => format!("Left: {left}\nRight: {right}"),
        _ => String::new(),
    };
    if !tooltip.is_empty() {
        response.clone().on_hover_ui(|ui| {
            ui.set_max_width(760.0);
            ui.label(RichText::new(tooltip).monospace().size(12.0));
        });
    }
    response.context_menu(|ui| {
        if let Some(left) = left {
            if common::intent_button(ui, "Copy left value", common::ButtonIntent::Ghost).clicked() {
                ui.ctx().copy_text(left.to_string());
                ui.close_menu();
            }
        }
        if let Some(right) = right {
            if common::intent_button(ui, "Copy right value", common::ButtonIntent::Ghost).clicked()
            {
                ui.ctx().copy_text(right.to_string());
                ui.close_menu();
            }
        }
        let both = match (left, right) {
            (Some(left), Some(right)) => format!("{left}\t{right}"),
            (Some(left), None) => left.to_string(),
            (None, Some(right)) => right.to_string(),
            (None, None) => String::new(),
        };
        if !both.is_empty()
            && common::intent_button(ui, "Copy both", common::ButtonIntent::Ghost).clicked()
        {
            ui.ctx().copy_text(both);
            ui.close_menu();
        }
        ui.separator();
        column_context_menu(ui, compare, col, state);
    });
    response
}

fn colored_job(
    text: &str,
    bg: Color32,
    color: Color32,
    query: &str,
    case_sensitive: bool,
) -> LayoutJob {
    let search_spans = if query.is_empty() {
        Vec::new()
    } else {
        find_ranges(text, query, case_sensitive)
    };
    let mut cuts = vec![0usize, text.len()];
    for span in &search_spans {
        let start = span.start.min(text.len());
        let end = span.end.min(text.len());
        if start < end && text.is_char_boundary(start) && text.is_char_boundary(end) {
            cuts.push(start);
            cuts.push(end);
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for pair in cuts.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start >= end {
            continue;
        }
        let highlighted = search_spans
            .iter()
            .any(|span| span.start < end && span.end > start);
        job.append(
            &text[start..end],
            0.0,
            TextFormat {
                font_id: FontId::monospace(12.0),
                color,
                background: if highlighted { theme::search_bg() } else { bg },
                ..Default::default()
            },
        );
    }
    job
}

fn paint_side_cell(ui: &mut Ui, cell: &SideCell<'_>, state: &mut CsvUiState) -> egui::Response {
    let compare = cell.compare;
    let row = cell.row;
    let col = cell.col;
    let value = cell.value;
    let present = cell.present;
    let kind = cell.kind;
    let left_side = cell.left_side;
    let changed = cell.changed;
    let spans = cell.spans;
    let selected = cell.selected;
    let selected_cell = cell.selected_cell;
    let query = cell.query;
    let case_sensitive = cell.case_sensitive;
    let tooltip = cell.tooltip;
    let rect = ui.max_rect();
    let mut bg = if selected || selected_cell {
        theme::sel_bg()
    } else if changed {
        theme::cell_bg(kind, left_side)
    } else {
        Color32::TRANSPARENT
    };
    if !present {
        bg = theme::gutter();
    }
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    if selected_cell {
        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0_f32, theme::accent()),
            egui::StrokeKind::Inside,
        );
    }
    if !present {
        return ui.interact(
            rect,
            ui.id()
                .with(("empty_csv_cell", rect.left() as i32, rect.top() as i32)),
            Sense::click(),
        );
    }
    let display = display_value(value);
    let search_spans = if query.is_empty() {
        Vec::new()
    } else {
        find_ranges(&display, query, case_sensitive)
    };
    let effective_spans: &[std::ops::Range<usize>] = if display.as_ref() == value {
        spans
    } else {
        &[]
    };
    let job = common::highlighted_job(
        &display,
        FontId::monospace(12.0),
        if changed {
            theme::text()
        } else {
            theme::muted()
        },
        effective_spans,
        if left_side {
            theme::del_strong()
        } else {
            theme::add_strong()
        },
        &search_spans,
        theme::search_bg(),
    );
    let response = ui.add(
        Label::new(job)
            .wrap_mode(TextWrapMode::Extend)
            .selectable(false)
            .sense(Sense::click()),
    );
    if !tooltip.is_empty() {
        response.clone().on_hover_ui(|ui| {
            ui.set_max_width(760.0);
            ui.label(RichText::new(tooltip).monospace().size(12.0));
        });
    }
    response.context_menu(|ui| {
        if common::intent_button(ui, "Copy value", common::ButtonIntent::Ghost).clicked() {
            ui.ctx().copy_text(value.to_string());
            ui.close_menu();
        }
        ui.separator();
        column_context_menu(ui, compare, col, state);
    });
    let _ = row;
    response
}
