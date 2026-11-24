//! Text/PDF comparison table rendering and cell painting.

use super::*;

pub(super) struct TextTableContext<'a> {
    pub view: &'a TextDiffView,
    pub selected_row: Option<usize>,
    pub selected_group: usize,
    pub scroll_row: Option<usize>,
    pub query: &'a str,
    pub case_sensitive: bool,
}

pub(super) struct PaneLayout<'a> {
    pub name: &'a str,
    pub right_side: bool,
    pub gutter: f32,
    pub text_width: f32,
    pub sync: egui::Vec2,
}

struct LinePaint<'a> {
    text: &'a str,
    spans: &'a [std::ops::Range<usize>],
    bg: Color32,
    query: &'a str,
    case_sensitive: bool,
    marker: &'a str,
}

struct TextCell<'a> {
    text: &'a str,
    spans: Option<&'a [std::ops::Range<usize>]>,
    focus_span: Option<&'a std::ops::Range<usize>>,
    kind: RowKind,
    right_side: bool,
    selected: bool,
    query: &'a str,
    case_sensitive: bool,
}

pub(super) fn render_text_pane(
    ui: &mut Ui,
    ctx: &TextTableContext<'_>,
    pane: &PaneLayout<'_>,
    state: &mut TextUiState,
) -> egui::Vec2 {
    let view = ctx.view;
    let selected_row = ctx.selected_row;
    let selected_group = ctx.selected_group;
    let scroll_row = ctx.scroll_row;
    let query = ctx.query;
    let case_sensitive = ctx.case_sensitive;
    let name = pane.name;
    let right_side = pane.right_side;
    let gutter = pane.gutter;
    let text_width = pane.text_width;
    let sync = pane.sync;
    let mut offset = sync;
    let content_width = gutter + text_width + 12.0;
    let area_output = egui::ScrollArea::horizontal()
        .id_salt(if right_side {
            "text_right_h"
        } else {
            "text_left_h"
        })
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .horizontal_scroll_offset(sync.x)
        .show(ui, |ui| {
            ui.set_min_width(content_width);
            let mut builder = TableBuilder::new(ui)
                .id_salt(if right_side {
                    "text_right_table"
                } else {
                    "text_left_table"
                })
                .striped(false)
                .resizable(false)
                .sense(Sense::click())
                .vertical_scroll_offset(sync.y)
                .column(Column::exact(gutter).clip(true))
                .column(Column::exact(text_width).clip(true));
            if let Some(row) = scroll_row {
                builder = builder.scroll_to_row(row, Some(Align::Center));
            }
            let table = builder.header(24.0, |mut header| {
                header.col(|ui| {
                    ui.label(
                        RichText::new(if right_side { "R" } else { "L" })
                            .color(if right_side {
                                theme::right()
                            } else {
                                theme::left()
                            })
                            .size(11.0)
                            .strong(),
                    );
                });
                header.col(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  ·  {} lines",
                            name,
                            if right_side {
                                view.stats.right_lines
                            } else {
                                view.stats.left_lines
                            }
                        ))
                        .color(if right_side {
                            theme::right()
                        } else {
                            theme::left()
                        })
                        .size(12.0)
                        .strong(),
                    );
                });
            });
            let output = table.body(|body| {
                body.rows(18.0, view.rows.len(), |mut row| {
                    let idx = row.index();
                    let diff_row = &view.rows[idx];
                    let selected = selected_row == Some(idx);
                    row.set_selected(selected);
                    let line = if right_side {
                        diff_row.right
                    } else {
                        diff_row.left
                    };
                    let line_no = line.map(|line| (line + 1).to_string()).unwrap_or_default();
                    row.col(|ui| paint_line_number(ui, &line_no, selected));
                    row.col(|ui| {
                        let (text, spans) = if right_side {
                            match diff_row.right {
                                Some(line) => {
                                    (view.right_line(line), Some(diff_row.right_spans.as_slice()))
                                }
                                None => ("", None),
                            }
                        } else {
                            match diff_row.left {
                                Some(line) => {
                                    (view.left_line(line), Some(diff_row.left_spans.as_slice()))
                                }
                                None => ("", None),
                            }
                        };
                        let focus_span = if selected {
                            let groups = if right_side {
                                &diff_row.group_right
                            } else {
                                &diff_row.group_left
                            };
                            groups.get(selected_group).and_then(|range| range.as_ref())
                        } else {
                            None
                        };
                        paint_text_cell(
                            ui,
                            &TextCell {
                                text,
                                spans,
                                focus_span,
                                kind: diff_row.kind,
                                right_side,
                                selected,
                                query,
                                case_sensitive,
                            },
                        );
                    });
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

pub(super) fn merged_text_table(
    ui: &mut Ui,
    ctx: &TextTableContext<'_>,
    left_name: &str,
    right_name: &str,
    state: &mut TextUiState,
) {
    let view = ctx.view;
    let scroll_row = ctx.scroll_row;
    let selected_group = ctx.selected_group;
    let query = ctx.query;
    let case_sensitive = ctx.case_sensitive;
    let glyph = ui.fonts(|fonts| fonts.glyph_width(&FontId::monospace(12.5), 'M'));
    let width_chars = state.left_width_chars.max(state.right_width_chars);
    let text_w = ((width_chars as f32 * glyph) + 48.0).clamp(320.0, 12000.0);
    let gutter = 96.0;
    let mut initial_x = state.sync_scroll_x;
    if let Some(ch) = state.scroll_to_char.take() {
        let viewport = ui.available_width().max(160.0);
        initial_x = common::char_scroll_target(ch, gutter, glyph, viewport);
        state.sync_scroll_x = initial_x;
    }
    let heights: Vec<f32> = view
        .rows
        .iter()
        .map(|row| {
            if row.kind == RowKind::Modified {
                36.0
            } else {
                20.0
            }
        })
        .collect();
    let area_output = egui::ScrollArea::horizontal()
        .id_salt("text_merged_h")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .horizontal_scroll_offset(initial_x)
        .show(ui, |ui| {
            ui.set_width(gutter + text_w + 24.0);
            let mut builder = TableBuilder::new(ui)
                .id_salt("text_merged")
                .striped(false)
                .resizable(false)
                .sense(Sense::click())
                .column(Column::exact(gutter).clip(true))
                .column(Column::exact(text_w).clip(true));
            if let Some(row) = scroll_row {
                builder = builder.scroll_to_row(row, Some(Align::Center));
            }
            let table = builder.header(24.0, |mut header| {
                header.col(|ui| {
                    ui.label(
                        RichText::new("L / R")
                            .color(theme::muted())
                            .size(10.5)
                            .strong(),
                    );
                });
                header.col(|ui| {
                    ui.label(
                        RichText::new(format!("{left_name}  ↔  {right_name}"))
                            .color(theme::text())
                            .size(12.0)
                            .strong(),
                    );
                });
            });
            table.body(|body| {
                body.heterogeneous_rows(heights.into_iter(), |mut row| {
                    let idx = row.index();
                    let selected = state.selected_row == Some(idx);
                    row.set_selected(selected);
                    row.col(|ui| {
                        paint_merged_line_numbers(ui, view, idx, selected);
                    });
                    row.col(|ui| {
                        paint_merged_text_row(
                            ui,
                            view,
                            idx,
                            selected,
                            selected_group,
                            query,
                            case_sensitive,
                        );
                    });
                    if row.response().clicked() {
                        state.selected_row = Some(idx);
                    }
                });
            });
        });
    state.sync_scroll_x = area_output.state.offset.x;
}

fn paint_merged_line_numbers(ui: &mut Ui, view: &TextDiffView, row: usize, selected: bool) {
    let diff_row = &view.rows[row];
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
    let left = diff_row.left.map(|line| line + 1);
    let right = diff_row.right.map(|line| line + 1);
    let text = match (left, right) {
        (Some(left), Some(right)) => format!("{left} | {right}"),
        (Some(left), None) => format!("{left} |"),
        (None, Some(right)) => format!("| {right}"),
        (None, None) => String::new(),
    };
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        text,
        FontId::monospace(10.0),
        if selected {
            theme::text()
        } else {
            theme::faint()
        },
    );
}

fn paint_merged_text_row(
    ui: &mut Ui,
    view: &TextDiffView,
    row: usize,
    selected: bool,
    selected_group: usize,
    query: &str,
    case_sensitive: bool,
) {
    let diff_row = &view.rows[row];
    let rect = ui.max_rect();
    if selected {
        ui.painter().rect_filled(rect, 0.0, theme::sel_bg());
    }
    match diff_row.kind {
        RowKind::Equal => {
            if let Some(line) = diff_row.left.or(diff_row.right) {
                let text = if diff_row.left.is_some() {
                    view.left_line(line)
                } else {
                    view.right_line(line)
                };
                paint_merged_line(
                    ui,
                    rect,
                    &LinePaint {
                        text,
                        spans: &diff_row.left_spans,
                        bg: if selected {
                            Color32::TRANSPARENT
                        } else {
                            theme::row_bg(RowKind::Equal, false)
                        },
                        query,
                        case_sensitive,
                        marker: "",
                    },
                );
            }
        }
        RowKind::Modified => {
            let top = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.center().y));
            let bottom =
                egui::Rect::from_min_max(egui::pos2(rect.left(), rect.center().y), rect.max);
            if let Some(line) = diff_row.left {
                paint_merged_line(
                    ui,
                    top,
                    &LinePaint {
                        text: view.left_line(line),
                        spans: &diff_row.left_spans,
                        bg: if selected {
                            Color32::TRANSPARENT
                        } else {
                            theme::row_bg(RowKind::Modified, false)
                        },
                        query,
                        case_sensitive,
                        marker: "−",
                    },
                );
            }
            if let Some(line) = diff_row.right {
                paint_merged_line(
                    ui,
                    bottom,
                    &LinePaint {
                        text: view.right_line(line),
                        spans: &diff_row.right_spans,
                        bg: if selected {
                            Color32::TRANSPARENT
                        } else {
                            theme::row_bg(RowKind::Modified, true)
                        },
                        query,
                        case_sensitive,
                        marker: "+",
                    },
                );
            }
        }
        RowKind::LeftOnly => {
            if let Some(line) = diff_row.left {
                paint_merged_line(
                    ui,
                    rect,
                    &LinePaint {
                        text: view.left_line(line),
                        spans: &diff_row.left_spans,
                        bg: if selected {
                            Color32::TRANSPARENT
                        } else {
                            theme::row_bg(RowKind::LeftOnly, false)
                        },
                        query,
                        case_sensitive,
                        marker: "−",
                    },
                );
            }
        }
        RowKind::RightOnly => {
            if let Some(line) = diff_row.right {
                paint_merged_line(
                    ui,
                    rect,
                    &LinePaint {
                        text: view.right_line(line),
                        spans: &diff_row.right_spans,
                        bg: if selected {
                            Color32::TRANSPARENT
                        } else {
                            theme::row_bg(RowKind::RightOnly, true)
                        },
                        query,
                        case_sensitive,
                        marker: "+",
                    },
                );
            }
        }
    }
    if selected {
        let marker_offset = if diff_row.kind == RowKind::Equal {
            0.0
        } else {
            12.0
        };
        let text_x = rect.left() + 6.0 + marker_offset;
        if diff_row.kind == RowKind::Modified {
            let top_y = rect.top() + rect.height() * 0.25;
            let bottom_y = rect.top() + rect.height() * 0.75;
            if let Some(range) = diff_row
                .group_left
                .get(selected_group)
                .and_then(|range| range.as_ref())
            {
                paint_span_focus(ui, text_x, top_y, range);
            }
            if let Some(range) = diff_row
                .group_right
                .get(selected_group)
                .and_then(|range| range.as_ref())
            {
                paint_span_focus(ui, text_x, bottom_y, range);
            }
        }
    }
}

fn paint_merged_line(ui: &mut Ui, rect: egui::Rect, paint: &LinePaint<'_>) {
    let text = paint.text;
    let spans = paint.spans;
    let bg = paint.bg;
    let query = paint.query;
    let case_sensitive = paint.case_sensitive;
    let marker = paint.marker;
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    let search_spans = if query.is_empty() {
        Vec::new()
    } else {
        find_ranges(text, query, case_sensitive)
    };
    let diff_bg = if marker.starts_with('+') {
        theme::add_strong()
    } else {
        theme::del_strong()
    };
    let job = common::highlighted_job(
        text,
        FontId::monospace(12.5),
        theme::text(),
        spans,
        diff_bg,
        &search_spans,
        theme::search_bg(),
    );
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let mut x = rect.left() + 6.0;
    if !marker.is_empty() {
        ui.painter().text(
            egui::pos2(rect.left() + 6.0, rect.center().y),
            Align2::LEFT_CENTER,
            marker,
            FontId::monospace(11.0),
            if marker.starts_with('+') {
                theme::right()
            } else {
                theme::left()
            },
        );
        x += 12.0;
    }
    let y = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(egui::pos2(x, y), galley, theme::text());
}

/// Draw a bright outline around the focused intra-line change group.
fn paint_span_focus(ui: &mut Ui, text_x: f32, center_y: f32, focus: &std::ops::Range<usize>) {
    let font = FontId::monospace(12.5);
    let glyph = ui.fonts(|fonts| fonts.glyph_width(&font, 'M'));
    let x = text_x + focus.start as f32 * glyph;
    ui.painter().vline(
        x,
        (center_y - 8.0)..=(center_y + 8.0),
        Stroke::new(2.0_f32, theme::accent()),
    );
}

fn paint_line_number(ui: &mut Ui, number: &str, selected: bool) {
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
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        number,
        FontId::monospace(11.0),
        if selected {
            theme::text()
        } else {
            theme::faint()
        },
    );
}

fn paint_text_cell(ui: &mut Ui, cell: &TextCell<'_>) {
    let text = cell.text;
    let spans = cell.spans;
    let focus_span = cell.focus_span;
    let kind = cell.kind;
    let right_side = cell.right_side;
    let selected = cell.selected;
    let query = cell.query;
    let case_sensitive = cell.case_sensitive;
    let rect = ui.max_rect();
    let bg = if selected {
        theme::sel_bg()
    } else {
        theme::row_bg(kind, right_side)
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    if text.is_empty() {
        return;
    }
    let search_spans = if query.is_empty() {
        Vec::new()
    } else {
        find_ranges(text, query, case_sensitive)
    };
    let diff_bg = if right_side {
        theme::add_strong()
    } else {
        theme::del_strong()
    };
    let job = common::highlighted_job(
        text,
        FontId::monospace(12.5),
        theme::text(),
        spans.unwrap_or(&[]),
        diff_bg,
        &search_spans,
        theme::search_bg(),
    );
    let label = Label::new(job)
        .wrap_mode(TextWrapMode::Extend)
        .selectable(false)
        .sense(Sense::click());
    let response = ui.add(label);
    if let Some(focus) = focus_span {
        paint_span_focus(ui, rect.left(), rect.center().y, focus);
    }
    if text.chars().count() > 180 {
        let full = text.to_string();
        let _ = response.clone().on_hover_ui(|ui| {
            ui.set_max_width(760.0);
            ui.label(RichText::new(full).monospace().size(12.0));
        });
    }
    response.context_menu(|ui| {
        if common::icon_button(ui, Icon::Copy, "Copy line", common::ButtonIntent::Ghost).clicked() {
            ui.ctx().copy_text(text.to_string());
            ui.close_menu();
        }
    });
}
