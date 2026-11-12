//! CSV comparison view, layouts, navigation and column actions.

use crate::actions::{ExportKind, UiAction};
use crate::csvdiff::{
    column_backups, CsvAlign, CsvCompare, CsvIgnore, CsvOptions, CsvRowKind, DelimiterChoice,
};
use crate::load::OpenData;
use crate::textdiff::find_ranges;
use crate::ui::common::{self, SearchUiState, SearchUpdate};
use crate::ui::theme;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Label, RichText, Sense, Stroke, TextWrapMode, Ui,
};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;
use std::collections::BTreeSet;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CsvViewMode {
    #[default]
    Merged,
    SideBySide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvDeleteTarget {
    Left,
    Right,
    Both,
}

#[derive(Debug)]
pub struct CsvUiState {
    pub mode: CsvViewMode,
    pub differences_only: bool,
    pub selected_row: Option<usize>,
    pub selected_cell: Option<(usize, usize)>,
    pub scroll_to_row: Option<usize>,
    pub current_change: Option<usize>,
    pub search: SearchUiState,
    pub column_widths: Vec<f32>,
    pub hidden_columns: BTreeSet<usize>,
    pub hidden_dirty: bool,
    pub pending_key: Option<usize>,
    pub pending_delete: Option<(CsvDeleteTarget, usize)>,
    pub scroll_to_col: Option<usize>,
    pub focus_frames: u8,
    pub merged_scroll_x: f32,
    pub sync_scroll_x: f32,
    pub sync_scroll_y: f32,
    pub options_collapsed: bool,
}

impl CsvUiState {
    pub fn new(compare: &CsvCompare) -> Self {
        let first_change = compare.rows.iter().position(|row| row.kind.is_change());
        let first_cell = first_change.and_then(|row| changed_cell(compare, row));
        let mut state = Self {
            mode: CsvViewMode::Merged,
            differences_only: false,
            selected_row: first_change,
            selected_cell: first_cell.map(|col| (first_change.unwrap_or(0), col)),
            scroll_to_row: first_change,
            current_change: first_change,
            search: SearchUiState::default(),
            column_widths: Vec::new(),
            hidden_columns: BTreeSet::new(),
            hidden_dirty: false,
            pending_key: None,
            pending_delete: None,
            scroll_to_col: first_cell,
            focus_frames: if first_cell.is_some() { 2 } else { 0 },
            merged_scroll_x: 0.0,
            sync_scroll_x: 0.0,
            sync_scroll_y: 0.0,
            options_collapsed: false,
        };
        state.ensure_widths(compare);
        state
    }

    pub fn set_hidden(&mut self, ignore: &CsvIgnore) {
        self.hidden_columns = ignore.left.union(&ignore.right).copied().collect();
        self.hidden_dirty = false;
    }

    pub fn hidden_count(&self) -> usize {
        self.hidden_columns.len()
    }

    pub fn visible_columns(&self, compare: &CsvCompare) -> Vec<usize> {
        (0..compare.column_count)
            .filter(|col| !self.hidden_columns.contains(col))
            .collect()
    }

    pub fn ensure_widths(&mut self, compare: &CsvCompare) {
        if self.column_widths.len() == compare.column_count {
            return;
        }
        self.column_widths = compute_widths(compare);
    }

    pub fn refresh_search(&mut self, compare: &CsvCompare) {
        let query = if self.search.query.is_empty() {
            String::new()
        } else {
            self.search.query.clone()
        };
        if query.is_empty() {
            self.search.set_matches(Vec::new(), 0);
            return;
        }
        let mut matches = Vec::new();
        let mut count = 0usize;
        for (idx, row) in compare.rows.iter().enumerate() {
            let mut hit = false;
            if let Some(left) = row.left {
                for col in 0..compare.column_count {
                    if self.hidden_columns.contains(&col) {
                        continue;
                    }
                    let hits = find_ranges(
                        compare.left.cell(left, col),
                        &query,
                        self.search.case_sensitive,
                    )
                    .len();
                    count += hits;
                    hit |= hits > 0;
                }
            }
            if let Some(right) = row.right {
                for col in 0..compare.column_count {
                    if self.hidden_columns.contains(&col) {
                        continue;
                    }
                    let hits = find_ranges(
                        compare.right.cell(right, col),
                        &query,
                        self.search.case_sensitive,
                    )
                    .len();
                    count += hits;
                    hit |= hits > 0;
                }
            }
            if hit {
                matches.push(idx);
            }
        }
        if let Some(row) = self.search.set_matches(matches, count) {
            self.selected_row = Some(row);
            self.scroll_to_row = Some(row);
        }
    }

    pub fn goto_change(&mut self, compare: &CsvCompare, forward: bool) -> bool {
        let current = self.current_change.or(self.selected_row);

        // First walk the changed cells inside the current row, so a row with
        // several differing cells is reported cell by cell.
        if let Some(row) = current {
            if let Some(diff_row) = compare.rows.get(row) {
                let columns = &diff_row.changed_cols;
                if !columns.is_empty() {
                    let current_col = self
                        .selected_cell
                        .filter(|(selected_row, _)| *selected_row == row)
                        .map(|(_, col)| col);
                    let next = match current_col {
                        Some(col) if forward => {
                            columns.iter().copied().find(|candidate| *candidate > col)
                        }
                        Some(col) => columns
                            .iter()
                            .copied()
                            .rev()
                            .find(|candidate| *candidate < col),
                        None if forward => columns.first().copied(),
                        None => columns.last().copied(),
                    };
                    if let Some(col) = next {
                        self.select_cell(row, col);
                        return true;
                    }
                }
            }
        }

        // Then move to the next/previous row, wrapping inside the file. Coming
        // from a later cell, backward navigation lands on the last changed cell.
        let target = if forward {
            compare
                .next_change(current)
                .or_else(|| compare.next_change(None))
        } else {
            compare
                .previous_change(current)
                .or_else(|| compare.previous_change(None))
        };
        if let Some(row) = target {
            let column = if forward {
                changed_cell(compare, row)
            } else {
                changed_cell_last(compare, row)
            }
            .or_else(|| self.visible_columns(compare).first().copied())
            .unwrap_or(0);
            self.select_cell(row, column);
            true
        } else {
            false
        }
    }

    fn select_cell(&mut self, row: usize, column: usize) {
        self.current_change = Some(row);
        self.selected_row = Some(row);
        self.selected_cell = Some((row, column));
        self.scroll_to_row = Some(row);
        self.scroll_to_col = Some(column);
        self.focus_frames = 2;
    }
}

/// The first cell that actually differs in `row`, if any.
fn changed_cell(compare: &CsvCompare, row: usize) -> Option<usize> {
    let diff_row = compare.rows.get(row)?;
    diff_row
        .changed_cols
        .first()
        .copied()
        .or_else(|| diff_row.cells.first().map(|cell| cell.col))
}

/// The last cell that differs in `row`, if any.
fn changed_cell_last(compare: &CsvCompare, row: usize) -> Option<usize> {
    let diff_row = compare.rows.get(row)?;
    diff_row
        .changed_cols
        .last()
        .copied()
        .or_else(|| diff_row.cells.last().map(|cell| cell.col))
}

/// Position of the focused cell among all changed cells, and the total number
/// of changed cells. Rows without cell-level changes count as one change.
fn change_progress(compare: &CsvCompare, state: &CsvUiState) -> (usize, usize) {
    let selected = state.selected_cell.or_else(|| {
        state
            .current_change
            .map(|row| (row, changed_cell(compare, row).unwrap_or(0)))
    });
    let mut total = 0usize;
    let mut current = 0usize;
    for (row_idx, row) in compare.rows.iter().enumerate() {
        if !row.kind.is_change() {
            continue;
        }
        let count = row.changed_cols.len().max(1);
        if let Some((selected_row, selected_col)) = selected {
            if selected_row == row_idx {
                let offset = row
                    .changed_cols
                    .iter()
                    .position(|col| *col >= selected_col)
                    .map(|pos| pos + 1)
                    .unwrap_or(1);
                current = total + offset.min(count);
            }
        }
        total += count;
    }
    if total == 0 {
        (0, 0)
    } else {
        (current.max(1), total)
    }
}

fn compute_widths(compare: &CsvCompare) -> Vec<f32> {
    let mut widths = Vec::with_capacity(compare.column_count);
    for col in 0..compare.column_count {
        let mut max_chars = compare.column_name(col).width().min(40);
        for row in compare.rows.iter().take(5000) {
            let left_len = row
                .left
                .map(|line| display_value(compare.left.cell(line, col)).width())
                .unwrap_or(0);
            let right_len = row
                .right
                .map(|line| display_value(compare.right.cell(line, col)).width())
                .unwrap_or(0);
            let width_chars = left_len.max(right_len);
            max_chars = max_chars.max(width_chars.min(80));
        }
        widths.push((max_chars as f32 * 7.1 + 24.0).clamp(84.0, 320.0));
    }
    widths
}

pub fn show(
    ui: &mut Ui,
    compare: &CsvCompare,
    state: &mut CsvUiState,
    data: &OpenData,
    options: &mut CsvOptions,
    actions: &mut Vec<UiAction>,
    can_undo: bool,
) -> bool {
    let mut reload = false;
    state.ensure_widths(compare);

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(data.rel.as_str()).strong().size(14.0));
        ui.label(
            RichText::new("CSV")
                .color(theme::accent())
                .size(10.0)
                .strong(),
        );
        ui.separator();
        common::stat_chip(
            ui,
            "rows",
            format!("{} / {}", compare.stats.left_rows, compare.stats.right_rows),
            theme::muted(),
        );
        common::stat_chip(
            ui,
            "modified",
            compare.stats.modified.to_string(),
            theme::diff(),
        );
        common::stat_chip(ui, "added", compare.stats.added.to_string(), theme::right());
        common::stat_chip(
            ui,
            "removed",
            compare.stats.removed.to_string(),
            theme::left(),
        );
        common::stat_chip(
            ui,
            "cells",
            compare.stats.changed_cells.to_string(),
            theme::accent(),
        );
        if compare.stats.header_changes > 0 {
            common::stat_chip(
                ui,
                "headers",
                compare.stats.header_changes.to_string(),
                theme::type_color(),
            );
        }
        ui.separator();
        if common::intent_button(ui, "◀ Change", common::ButtonIntent::Ghost)
            .on_hover_text("Previous difference (Shift+F7)")
            .clicked()
        {
            if !state.goto_change(compare, false) {
                actions.push(UiAction::NextFileDiff { forward: false });
            }
            ui.ctx().request_repaint();
        }
        if common::intent_button(ui, "Change ▶", common::ButtonIntent::Ghost)
            .on_hover_text("Next difference (F7)")
            .clicked()
        {
            if !state.goto_change(compare, true) {
                actions.push(UiAction::NextFileDiff { forward: true });
            }
            ui.ctx().request_repaint();
        }
        if compare.row_change_count() > 0 {
            let (current, total) = change_progress(compare, state);
            common::change_counter(ui, current, total);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            common::collapse_arrow(ui, &mut state.options_collapsed);
        });
    });

    if !state.options_collapsed {
        egui::Grid::new("csv_toolbar_grid")
            .num_columns(5)
            .spacing(egui::vec2(14.0, 6.0))
            .show(ui, |ui| {
                ui.selectable_value(&mut state.mode, CsvViewMode::Merged, "Merged");
                ui.checkbox(&mut options.has_headers, "First row is header");
                if data.left_path.is_some() {
                    if common::intent_button(ui, "A → B", common::ButtonIntent::Secondary).clicked()
                    {
                        actions.push(UiAction::Copy {
                            rel: data.rel.clone(),
                            from_left: true,
                        });
                    }
                } else {
                    ui.label("");
                }
                ui.menu_button(format!("Hidden columns ({})", state.hidden_count()), |ui| {
                    ui.set_min_width(330.0);
                    if state.hidden_columns.is_empty() {
                        common::small_muted(
                            ui,
                            "No hidden columns. Right-click a column header to hide one.",
                        );
                    } else {
                        let hidden_cols: Vec<usize> =
                            state.hidden_columns.iter().copied().collect();
                        for col in hidden_cols {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        col + 1,
                                        compare.column_name(col)
                                    ))
                                    .strong(),
                                );
                                if common::intent_button(
                                    ui,
                                    "Restore",
                                    common::ButtonIntent::Secondary,
                                )
                                .clicked()
                                {
                                    state.hidden_columns.remove(&col);
                                    state.hidden_dirty = true;
                                    ui.close_menu();
                                }
                            });
                        }
                        ui.separator();
                        if common::intent_button(
                            ui,
                            "Restore all hidden columns",
                            common::ButtonIntent::Secondary,
                        )
                        .clicked()
                        {
                            state.hidden_columns.clear();
                            state.hidden_dirty = true;
                            ui.close_menu();
                        }
                    }
                });
                if common::intent_button(ui, "Find", common::ButtonIntent::Ghost)
                    .on_hover_text("Find (Ctrl+F)")
                    .clicked()
                {
                    state.search.open_and_focus();
                }
                ui.end_row();

                ui.selectable_value(&mut state.mode, CsvViewMode::SideBySide, "Side by side");
                ui.checkbox(&mut options.ignore_case, "Ignore case");
                if data.right_path.is_some() {
                    if common::intent_button(ui, "B → A", common::ButtonIntent::Secondary).clicked()
                    {
                        actions.push(UiAction::Copy {
                            rel: data.rel.clone(),
                            from_left: false,
                        });
                    }
                } else {
                    ui.label("");
                }
                ui.menu_button("Restore deleted columns", |ui| {
                    ui.set_min_width(360.0);
                    let mut any = false;
                    for (side, path) in [
                        ("Left", data.left_path.as_deref()),
                        ("Right", data.right_path.as_deref()),
                    ] {
                        let Some(path) = path else { continue };
                        let backups = column_backups(path);
                        if backups.is_empty() {
                            continue;
                        }
                        any = true;
                        ui.label(
                            RichText::new(format!("{side} · {}", path.display()))
                                .strong()
                                .size(11.5),
                        );
                        for backup in backups.iter().take(12) {
                            let file_name = path
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            let stamp = backup
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default()
                                .trim_start_matches(&format!("{file_name}."))
                                .trim_end_matches(".hcompare.bak")
                                .to_string();
                            let label = format!("Restore {file_name} · {stamp}");
                            if common::intent_button(ui, &label, common::ButtonIntent::Secondary)
                                .clicked()
                            {
                                actions.push(UiAction::RestoreCsvBackup {
                                    path: path.to_path_buf(),
                                    backup: backup.clone(),
                                });
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                    }
                    if !any {
                        common::small_muted(ui, "No .hcompare.bak backups beside these CSV files.");
                    }
                });
                ui.menu_button("Format", |ui| {
                    ui.menu_button(format!("Delimiter: {}", options.delimiter.label()), |ui| {
                        for choice in DelimiterChoice::ALL {
                            if ui
                                .selectable_value(&mut options.delimiter, choice, choice.label())
                                .changed()
                            {
                                reload = true;
                            }
                        }
                    });
                    ui.menu_button(
                        format!("Row alignment: {}", options.align.short_label()),
                        |ui| {
                            if ui
                                .selectable_value(
                                    &mut options.align,
                                    CsvAlign::Auto,
                                    "Auto (first column if unique)",
                                )
                                .changed()
                            {
                                reload = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut options.align,
                                    CsvAlign::RowOrder,
                                    "Row order",
                                )
                                .changed()
                            {
                                reload = true;
                            }
                            ui.separator();
                            for col in 0..compare.column_count.min(40) {
                                let label = format!("Column {} (key)", col + 1);
                                if ui
                                    .selectable_value(&mut options.align, CsvAlign::Key(col), label)
                                    .changed()
                                {
                                    reload = true;
                                }
                            }
                        },
                    );
                });
                ui.end_row();

                ui.checkbox(&mut state.differences_only, "Differences only");
                ui.checkbox(&mut options.trim_whitespace, "Trim cells");
                ui.menu_button("Export", |ui| {
                    if common::intent_button(
                        ui,
                        "Export differences as CSV…",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::CsvDifferences));
                        ui.close_menu();
                    }
                    if common::intent_button(
                        ui,
                        "Export HTML report…",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::CsvHtml));
                        ui.close_menu();
                    }
                });
                if common::intent_button(ui, "Recompare", common::ButtonIntent::Secondary).clicked()
                {
                    reload = true;
                }
                ui.label("");
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut options.ignore_empty_rows, "Skip empty rows");
                ui.label("");
                if can_undo {
                    if common::intent_button(ui, "Undo delete", common::ButtonIntent::Ghost)
                        .on_hover_text("Restore the last deleted column (Ctrl+Z)")
                        .clicked()
                    {
                        actions.push(UiAction::UndoCsvColumnDelete);
                    }
                } else {
                    ui.label("");
                }
                ui.label("");
                ui.end_row();
            });

        ui.horizontal_wrapped(|ui| {
            common::small_muted(
                ui,
                format!(
                    "{} · {}",
                    crate::csvdiff::delimiter_label(compare.left.delimiter),
                    options.align.short_label()
                ),
            );
            if let CsvAlign::Key(key) = options.align {
                if key >= compare.column_count {
                    ui.label(
                        RichText::new("key out of range")
                            .color(theme::error())
                            .size(10.5),
                    );
                }
            }
        });
    }

    if state.search.open {
        let change = common::search_bar(ui, &mut state.search);
        if change == SearchUpdate::Changed || change == SearchUpdate::Navigate {
            state.refresh_search(compare);
        }
    }

    if !data.messages.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for message in &data.messages {
                ui.label(
                    RichText::new(format!("ⓘ {message}"))
                        .color(theme::diff())
                        .size(11.0),
                );
            }
        });
    }
    if compare.align != options.align {
        if let (CsvAlign::Auto, CsvAlign::Key(key)) = (options.align, compare.align) {
            common::small_muted(
                ui,
                format!("Auto-aligned by column {} (first unique column).", key + 1),
            );
        }
    }
    ui.separator();

    if compare.column_count == 0 {
        ui.centered_and_justified(|ui| {
            common::small_muted(
                ui,
                "No columns found. Check the delimiter and header settings.",
            );
        });
        return reload;
    }
    if compare.rows.is_empty() {
        ui.centered_and_justified(|ui| {
            common::small_muted(ui, "Both files have no data rows.");
        });
        return reload;
    }

    let visible = visible_rows(compare, state);
    if visible.is_empty() {
        ui.centered_and_justified(|ui| {
            common::small_muted(ui, "No rows match the current filter.");
        });
        return reload;
    }
    if let Some(selected) = state.selected_row {
        if !visible.contains(&selected) {
            state.selected_row = visible.first().copied();
        }
    }

    let query = state.search.query.clone();
    let case_sensitive = state.search.case_sensitive;
    let selected_row = state.selected_row;
    let selected_cell = state.selected_cell;
    let scroll_row = state
        .scroll_to_row
        .take()
        .and_then(|row| visible.iter().position(|visible_row| *visible_row == row));
    common::small_muted(
        ui,
        "Right-click a column to hide or delete it · panes share vertical and horizontal scroll",
    );
    let widths = state.column_widths.clone();
    let visible_columns = state.visible_columns(compare);
    let focus_col = state.scroll_to_col;
    if focus_col.is_some() {
        state.focus_frames = state.focus_frames.saturating_sub(1);
        if state.focus_frames == 0 {
            state.scroll_to_col = None;
        }
    }

    common::table_surface(ui);
    let table_ctx = CsvTableContext {
        compare,
        visible: &visible,
        columns: &visible_columns,
        widths: &widths,
        selected_row,
        selected_cell,
        scroll_row,
        focus_col,
        query: &query,
        case_sensitive,
    };
    match state.mode {
        CsvViewMode::Merged => merged_table(ui, &table_ctx, state),
        CsvViewMode::SideBySide => side_by_side_table(ui, &table_ctx, state),
    }

    ui.add_space(4.0);
    if state.hidden_dirty {
        state.hidden_dirty = false;
        actions.push(UiAction::RecomputeCsv {
            hidden: state.hidden_columns.iter().copied().collect(),
        });
    }
    if let Some(col) = state.pending_key.take() {
        options.align = CsvAlign::Key(col);
        reload = true;
    }
    if let Some((target, col)) = state.pending_delete.take() {
        match target {
            CsvDeleteTarget::Left => actions.push(UiAction::DeleteCsvColumn {
                left: true,
                column: col,
            }),
            CsvDeleteTarget::Right => actions.push(UiAction::DeleteCsvColumn {
                left: false,
                column: col,
            }),
            CsvDeleteTarget::Both => actions.push(UiAction::DeleteCsvColumnBoth { column: col }),
        }
    }
    reload
}

fn visible_rows(compare: &CsvCompare, state: &CsvUiState) -> Vec<usize> {
    if state.differences_only && state.search.query.is_empty() {
        compare
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.kind.is_change())
            .map(|(idx, _)| idx)
            .collect()
    } else {
        (0..compare.rows.len()).collect()
    }
}

fn display_value(value: &str) -> Cow<'_, str> {
    if !value
        .chars()
        .any(|ch| ch == '\n' || ch == '\r' || ch == '\t' || ch.is_control())
    {
        return Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len() + 8);
    for ch in value.chars() {
        match ch {
            '\n' => out.push('⏎'),
            '\r' => out.push('␍'),
            '\t' => out.push_str("    "),
            ch if ch.is_control() => out.push('·'),
            ch => out.push(ch),
        }
    }
    Cow::Owned(out)
}

mod menus;
mod table;
#[cfg(test)]
mod tests;

use table::*;
