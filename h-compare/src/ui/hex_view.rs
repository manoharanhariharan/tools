//! Side-by-side hexadecimal comparison for binary files.

use crate::load::BinaryBytes;
use crate::ui::common;
use crate::ui::icons::Icon;
use crate::ui::theme;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align, Color32, FontId, Label, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

pub struct HexUiState {
    pub bytes_per_row: usize,
    pub selected_offset: Option<usize>,
    pub scroll_to_row: Option<usize>,
    pub diff_count: usize,
    pub first_diff: Option<usize>,
}

impl HexUiState {
    pub fn new(data: &BinaryBytes) -> Self {
        let (diff_count, first_diff) = diff_stats(data);
        Self {
            bytes_per_row: 16,
            selected_offset: first_diff,
            scroll_to_row: first_diff.map(|offset| offset / 16),
            diff_count,
            first_diff,
        }
    }

    pub fn go_next(&mut self, data: &BinaryBytes) -> bool {
        let Some(offset) = next_diff(data, self.selected_offset) else {
            return false;
        };
        self.selected_offset = Some(offset);
        self.scroll_to_row = Some(offset / self.bytes_per_row);
        true
    }

    pub fn go_previous(&mut self, data: &BinaryBytes) -> bool {
        let Some(offset) = previous_diff(data, self.selected_offset) else {
            return false;
        };
        self.selected_offset = Some(offset);
        self.scroll_to_row = Some(offset / self.bytes_per_row);
        true
    }
}

fn diff_stats(data: &BinaryBytes) -> (usize, Option<usize>) {
    let max = data.left.len().max(data.right.len());
    let mut count = 0usize;
    let mut first = None;
    for offset in 0..max {
        if data.left.get(offset) != data.right.get(offset) {
            count += 1;
            if first.is_none() {
                first = Some(offset);
            }
        }
    }
    (count, first)
}

fn next_diff(data: &BinaryBytes, current: Option<usize>) -> Option<usize> {
    let max = data.left.len().max(data.right.len());
    let start = current.map(|offset| offset + 1).unwrap_or(0);
    (start..max).find(|offset| data.left.get(*offset) != data.right.get(*offset))
}

fn previous_diff(data: &BinaryBytes, current: Option<usize>) -> Option<usize> {
    let mut offset = current.unwrap_or(data.left.len().max(data.right.len()));
    while offset > 0 {
        offset -= 1;
        if data.left.get(offset) != data.right.get(offset) {
            return Some(offset);
        }
    }
    None
}

pub fn show(ui: &mut Ui, rel: &str, data: &BinaryBytes, state: &mut HexUiState) {
    let max_len = data.left.len().max(data.right.len());
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(rel).strong().size(14.0));
        ui.label(
            RichText::new("HEX")
                .color(theme::type_color())
                .size(10.0)
                .strong(),
        );
        ui.separator();
        common::stat_chip(
            ui,
            "bytes",
            format!("{} / {}", data.left.len(), data.right.len()),
            theme::muted(),
        );
        common::stat_chip(
            ui,
            "differing bytes",
            state.diff_count.to_string(),
            theme::diff(),
        );
        if let Some(first) = state.first_diff {
            common::stat_chip(ui, "first diff", format!("0x{first:X}"), theme::left());
        }
        if let Some(selected) = state.selected_offset {
            common::stat_chip(ui, "selected", format!("0x{selected:X}"), theme::accent());
        }
    });

    ui.horizontal_wrapped(|ui| {
        let _ = common::icon_menu_button(
            ui,
            Icon::Columns,
            &format!("Bytes/row: {}", state.bytes_per_row),
            |ui| {
                for value in [8usize, 16] {
                    if ui
                        .selectable_label(state.bytes_per_row == value, value.to_string())
                        .clicked()
                    {
                        state.bytes_per_row = value;
                        if let Some(selected) = state.selected_offset {
                            state.scroll_to_row = Some(selected / value);
                        }
                        common::close_menu(ui);
                    }
                }
            },
        );
        if common::icon_button(
            ui,
            Icon::ChevronLeft,
            "Previous",
            common::ButtonIntent::Ghost,
        )
        .on_hover_text("Previous differing byte (Shift+F7)")
        .clicked()
        {
            state.go_previous(data);
        }
        if common::icon_button(ui, Icon::ChevronRight, "Next", common::ButtonIntent::Ghost)
            .on_hover_text("Next differing byte (F7)")
            .clicked()
        {
            state.go_next(data);
        }
        ui.separator();
        common::small_muted(
            ui,
            "Click a row to select it · differing bytes are tinted left/right",
        );
    });

    if data.left_truncated || data.right_truncated {
        ui.label(
            RichText::new("Oversized binary detected: only the inspected prefix is shown in hex.")
                .color(theme::diff())
                .size(11.0),
        );
    }
    if max_len == 0 {
        ui.centered_and_justified(|ui| common::small_muted(ui, "Both binary files are empty."));
        return;
    }

    let bytes_per_row = state.bytes_per_row;
    let rows = max_len.div_ceil(bytes_per_row);
    let hex_chars = bytes_per_row * 3;
    let hex_width = (hex_chars as f32 * 7.1 + 24.0).clamp(180.0, 440.0);
    let total_width = 92.0 + hex_width * 2.0 + 40.0;
    ui.add_space(4.0);
    common::table_surface(ui);
    egui::ScrollArea::vertical()
        .id_salt("hex_rows")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(total_width);
            let mut builder = TableBuilder::new(ui)
                .id_salt("hex_table")
                .striped(false)
                .resizable(false)
                .sense(Sense::click())
                .column(Column::exact(92.0))
                .column(Column::exact(hex_width))
                .column(Column::exact(hex_width));
            if let Some(row) = state.scroll_to_row.take() {
                builder = builder.scroll_to_row(row, Some(Align::Center));
            }
            let table = builder.header(26.0, |mut header| {
                for title in ["OFFSET", "A · LEFT", "B · RIGHT"] {
                    header.col(|ui| {
                        ui.label(
                            RichText::new(title)
                                .color(theme::muted())
                                .size(10.5)
                                .strong(),
                        );
                    });
                }
            });
            table.body(|body| {
                body.rows(20.0, rows, |mut row| {
                    let row_index = row.index();
                    let start = row_index * bytes_per_row;
                    let selected = state
                        .selected_offset
                        .map(|offset| offset / bytes_per_row == row_index)
                        .unwrap_or(false);
                    row.set_selected(selected);
                    row.col(|ui| {
                        ui.add(
                            Label::new(
                                RichText::new(format!("{start:08X}"))
                                    .monospace()
                                    .size(11.0)
                                    .color(if selected {
                                        theme::text()
                                    } else {
                                        theme::muted()
                                    }),
                            )
                            .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        ui.add(
                            Label::new(hex_job(
                                &data.left,
                                &data.right,
                                start,
                                bytes_per_row,
                                true,
                            ))
                            .wrap_mode(egui::TextWrapMode::Extend)
                            .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        ui.add(
                            Label::new(hex_job(
                                &data.right,
                                &data.left,
                                start,
                                bytes_per_row,
                                false,
                            ))
                            .wrap_mode(egui::TextWrapMode::Extend)
                            .selectable(false),
                        );
                    });
                    let response = row.response();
                    if response.clicked() {
                        state.selected_offset = Some(start);
                    }
                    response.context_menu(|ui| {
                        if common::icon_button(
                            ui,
                            Icon::Copy,
                            &format!("Copy offset 0x{start:X}"),
                            common::ButtonIntent::Ghost,
                        )
                        .clicked()
                        {
                            ui.ctx().copy_text(format!("0x{start:X}"));
                            ui.close_menu();
                        }
                        if common::icon_button(
                            ui,
                            Icon::Copy,
                            "Copy left row (hex)",
                            common::ButtonIntent::Ghost,
                        )
                        .clicked()
                        {
                            ui.ctx()
                                .copy_text(row_hex(&data.left, start, bytes_per_row, true));
                            ui.close_menu();
                        }
                        if common::icon_button(
                            ui,
                            Icon::Copy,
                            "Copy right row (hex)",
                            common::ButtonIntent::Ghost,
                        )
                        .clicked()
                        {
                            ui.ctx()
                                .copy_text(row_hex(&data.right, start, bytes_per_row, true));
                            ui.close_menu();
                        }
                    });
                });
            });
        });
}

fn hex_job(
    bytes: &[u8],
    other: &[u8],
    start: usize,
    bytes_per_row: usize,
    left_side: bool,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for index in 0..bytes_per_row {
        let offset = start + index;
        let value = bytes.get(offset);
        let changed = value != other.get(offset);
        let background = if changed && value.is_some() {
            if left_side {
                theme::del_bg()
            } else {
                theme::add_bg()
            }
        } else {
            Color32::TRANSPARENT
        };
        let text = match value {
            Some(byte) => format!("{byte:02X} "),
            None => "   ".to_string(),
        };
        let mut format = TextFormat {
            font_id: FontId::monospace(12.0),
            color: if value.is_some() {
                theme::text()
            } else {
                theme::faint()
            },
            ..Default::default()
        };
        if background != Color32::TRANSPARENT {
            format.background = background;
        }
        job.append(&text, 0.0, format);
    }
    job
}

fn row_hex(bytes: &[u8], start: usize, bytes_per_row: usize, spaced: bool) -> String {
    let end = (start + bytes_per_row).min(bytes.len());
    let values: Vec<String> = bytes[start.min(end)..end]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();
    if spaced {
        values.join(" ")
    } else {
        values.concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(left: &[u8], right: &[u8]) -> BinaryBytes {
        BinaryBytes {
            left: left.to_vec(),
            right: right.to_vec(),
            left_truncated: false,
            right_truncated: false,
        }
    }

    #[test]
    fn finds_and_navigates_differences() {
        let bytes = data(&[0, 1, 2, 3, 4], &[0, 9, 2, 3, 8]);
        let mut state = HexUiState::new(&bytes);
        assert_eq!(state.diff_count, 2);
        assert_eq!(state.first_diff, Some(1));
        assert_eq!(state.selected_offset, Some(1));
        assert!(state.go_next(&bytes));
        assert_eq!(state.selected_offset, Some(4));
        assert!(!state.go_next(&bytes));
        assert!(state.go_previous(&bytes));
        assert_eq!(state.selected_offset, Some(1));
        assert!(!state.go_previous(&bytes));
    }

    #[test]
    fn counts_size_differences_as_changed_bytes() {
        let bytes = data(&[1, 2, 3], &[1, 2]);
        let state = HexUiState::new(&bytes);
        assert_eq!(state.diff_count, 1);
        assert_eq!(state.first_diff, Some(2));
    }

    #[test]
    fn row_hex_formats_available_bytes() {
        assert_eq!(
            row_hex(&[0xDE, 0xAD, 0xBE, 0xEF], 0, 4, true),
            "DE AD BE EF"
        );
        assert_eq!(row_hex(&[0xDE, 0xAD], 0, 4, true), "DE AD");
        assert_eq!(row_hex(&[0xDE, 0xAD], 4, 4, true), "");
    }
}
