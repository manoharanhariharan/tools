//! CSV comparison view, layouts, navigation and column actions.

use crate::actions::{ExportKind, UiAction};
use crate::csvdiff::{CsvAlign, CsvCompare, CsvIgnore, CsvOptions, CsvRowKind, DelimiterChoice};
use crate::load::OpenData;
use crate::textdiff::find_ranges;
use crate::ui::common::{self, SearchUiState, SearchUpdate};
use crate::ui::icons::Icon;
use crate::ui::theme;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Label, RichText, Sense, Stroke, TextWrapMode, Ui,
};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::PathBuf;
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
    /// Columns explicitly shown again while "Differences only" is active.
    pub shown_columns: BTreeSet<usize>,
    pub hidden_dirty: bool,
    /// Backups already restored in this session; hidden from the restore menu.
    pub restored_backups: BTreeSet<PathBuf>,
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
            shown_columns: BTreeSet::new(),
            hidden_dirty: false,
            restored_backups: BTreeSet::new(),
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

    pub fn visible_columns(&self, compare: &CsvCompare) -> Vec<usize> {
        (0..compare.column_count)
            .filter(|col| self.column_visible(compare, *col))
            .collect()
    }

    /// Whether a column should be drawn right now. "Differences only"
    /// auto-hides unchanged columns; an explicit show from the Columns menu
    /// overrides that until the user hides it again.
    pub fn column_visible(&self, compare: &CsvCompare, col: usize) -> bool {
        if self.hidden_columns.contains(&col) {
            return false;
        }
        if self.differences_only
            && !self.shown_columns.contains(&col)
            && !compare.column_has_differences(col)
        {
            return false;
        }
        true
    }

    /// Apply a visibility toggle from the Columns menu or a header menu.
    pub fn set_column_visible(&mut self, compare: &CsvCompare, col: usize, visible: bool) {
        if visible {
            let was_hidden = self.hidden_columns.remove(&col);
            if self.differences_only && !compare.column_has_differences(col) {
                self.shown_columns.insert(col);
            } else {
                self.shown_columns.remove(&col);
            }
            if was_hidden {
                self.hidden_dirty = true;
            }
        } else if self.differences_only
            && !compare.column_has_differences(col)
            && self.shown_columns.contains(&col)
        {
            // Unchecking an auto-shown unchanged column just returns it to the
            // differences-only hidden group; it is not an explicit hide.
            self.shown_columns.remove(&col);
        } else {
            self.hidden_columns.insert(col);
            self.shown_columns.remove(&col);
            self.hidden_dirty = true;
        }
    }

    /// Split columns into the visible group (checked, in column order) and the
    /// hidden group (unchecked, shown below the visible ones).
    pub fn column_visibility_entries(&self, compare: &CsvCompare) -> (Vec<usize>, Vec<usize>) {
        let mut visible = Vec::with_capacity(compare.column_count);
        let mut hidden = Vec::new();
        for col in 0..compare.column_count {
            if self.column_visible(compare, col) {
                visible.push(col);
            } else {
                hidden.push(col);
            }
        }
        (visible, hidden)
    }

    /// Move the selected cell with the keyboard. Up/Down walk the currently
    /// visible rows, Left/Right walk the visible columns, and Page/Home/End
    /// jump by ten rows or to the first/last column.
    pub fn navigate_cells(
        &mut self,
        compare: &CsvCompare,
        visible: &[usize],
        key: common::NavKey,
    ) -> bool {
        if visible.is_empty() {
            return false;
        }
        let columns = self.visible_columns(compare);
        if columns.is_empty() {
            return false;
        }
        let row_pos = self
            .selected_row
            .and_then(|row| visible.iter().position(|candidate| *candidate == row))
            .unwrap_or(0);
        let col_pos = self
            .selected_cell
            .filter(|(row, _)| Some(*row) == self.selected_row)
            .and_then(|(_, col)| columns.iter().position(|candidate| *candidate == col))
            .unwrap_or(0);
        const PAGE_STEP: usize = 10;
        let (row_pos, col_pos) = match key {
            common::NavKey::Up => (row_pos.saturating_sub(1), col_pos),
            common::NavKey::Down => ((row_pos + 1).min(visible.len() - 1), col_pos),
            common::NavKey::Left => (row_pos, col_pos.saturating_sub(1)),
            common::NavKey::Right => (row_pos, (col_pos + 1).min(columns.len() - 1)),
            common::NavKey::Home => (row_pos, 0),
            common::NavKey::End => (row_pos, columns.len() - 1),
            common::NavKey::PageUp => (row_pos.saturating_sub(PAGE_STEP), col_pos),
            common::NavKey::PageDown => ((row_pos + PAGE_STEP).min(visible.len() - 1), col_pos),
        };
        let row = visible[row_pos];
        let col = columns[col_pos];
        if self.selected_row == Some(row) && self.selected_cell == Some((row, col)) {
            return false;
        }
        self.select_cell(row, col);
        true
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
                    if !self.column_visible(compare, col) {
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
                    if !self.column_visible(compare, col) {
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
                .map(|line| {
                    display_value_capped(compare.left.cell(line, col), MEASURE_VALUE_CHARS).width()
                })
                .unwrap_or(0);
            let right_len = row
                .right
                .map(|line| {
                    display_value_capped(compare.right.cell(line, col), MEASURE_VALUE_CHARS).width()
                })
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
        let (current, total) = change_progress(compare, state);
        if let Some(forward) = common::change_nav(ui, current, total) {
            if !state.goto_change(compare, forward) {
                actions.push(UiAction::NextFileDiff { forward });
            }
            ui.ctx().request_repaint();
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
                let mut mode = state.mode;
                common::view_menu(
                    ui,
                    &mut mode,
                    &[
                        (CsvViewMode::Merged, "Merged"),
                        (CsvViewMode::SideBySide, "Side by side"),
                    ],
                );
                state.mode = mode;
                ui.checkbox(&mut options.has_headers, "First row is header");
                if data.left_path.is_some() {
                    if common::icon_button(ui, Icon::Copy, "A → B", common::ButtonIntent::Secondary)
                        .clicked()
                    {
                        actions.push(UiAction::Copy {
                            rel: data.rel.clone(),
                            from_left: true,
                        });
                    }
                } else {
                    ui.label("");
                }
                menus::columns_menu(ui, compare, state);
                common::find_button(ui, &mut state.search);
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut options.ignore_case, "Ignore case");
                if data.right_path.is_some() {
                    if common::icon_button(ui, Icon::Copy, "B → A", common::ButtonIntent::Secondary)
                        .clicked()
                    {
                        actions.push(UiAction::Copy {
                            rel: data.rel.clone(),
                            from_left: false,
                        });
                    }
                } else {
                    ui.label("");
                }
                menus::restore_columns_menu(ui, data, &state.restored_backups, actions);
                let _ = common::icon_menu_button(ui, Icon::Sliders, "Format", |ui| {
                    ui.set_min_width(300.0);
                    common::section_heading(ui, "DELIMITER");
                    for choice in DelimiterChoice::ALL {
                        if ui
                            .selectable_value(&mut options.delimiter, choice, choice.label())
                            .changed()
                        {
                            reload = true;
                            common::close_menu(ui);
                        }
                    }
                    ui.separator();
                    common::section_heading(ui, "ROW ALIGNMENT");
                    for (value, label) in [
                        (CsvAlign::Auto, "Auto (first column if unique)"),
                        (CsvAlign::RowOrder, "Row order"),
                    ] {
                        if ui
                            .selectable_value(&mut options.align, value, label)
                            .changed()
                        {
                            reload = true;
                            common::close_menu(ui);
                        }
                    }
                    ui.separator();
                    for col in 0..compare.column_count.min(40) {
                        let label = format!("Column {} (key)", col + 1);
                        if ui
                            .selectable_value(&mut options.align, CsvAlign::Key(col), label)
                            .changed()
                        {
                            reload = true;
                            common::close_menu(ui);
                        }
                    }
                });
                ui.end_row();

                if ui
                    .checkbox(&mut state.differences_only, "Differences only")
                    .changed()
                {
                    state.refresh_search(compare);
                }
                ui.checkbox(&mut options.trim_whitespace, "Trim cells");
                let _ = common::icon_menu_button(ui, Icon::Download, "Export", |ui| {
                    if common::icon_button(
                        ui,
                        Icon::Download,
                        "Differences as CSV…",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::CsvDifferences));
                        common::close_menu(ui);
                    }
                    if common::icon_button(
                        ui,
                        Icon::Download,
                        "HTML report…",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::CsvHtml));
                        common::close_menu(ui);
                    }
                });
                if common::icon_button(
                    ui,
                    Icon::Refresh,
                    "Recompare",
                    common::ButtonIntent::Secondary,
                )
                .clicked()
                {
                    reload = true;
                }
                ui.label("");
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut options.ignore_empty_rows, "Skip empty rows");
                ui.label("");
                if can_undo {
                    if common::icon_button(
                        ui,
                        Icon::Undo,
                        "Undo delete",
                        common::ButtonIntent::Ghost,
                    )
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
    if let Some((row, col)) = state.selected_cell {
        if !state.column_visible(compare, col) {
            state.selected_cell = state
                .visible_columns(compare)
                .first()
                .map(|first| (row, *first));
        }
    }
    if let Some(key) = common::take_navigation_key(ui) {
        if state.navigate_cells(compare, &visible, key) {
            ui.ctx().request_repaint();
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
        "Arrow keys move the selected cell · right-click a column to hide or delete it · panes share vertical and horizontal scroll",
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

/// Bounded display value used for table rendering and width measurement.
/// Very large CSV cells are truncated for layout while the full value remains
/// available in tooltips, search and exports.
fn display_value_capped(value: &str, limit: usize) -> Cow<'_, str> {
    let cut = value.char_indices().nth(limit).map(|(index, _)| index);
    let Some(index) = cut else {
        return display_value(value);
    };
    let mut out = display_value(&value[..index]).into_owned();
    out.push('…');
    Cow::Owned(out)
}

const MEASURE_VALUE_CHARS: usize = 160;
const RENDER_VALUE_CHARS: usize = 800;
const TOOLTIP_VALUE_CHARS: usize = 4000;

mod menus;
mod table;
#[cfg(test)]
mod tests;

use table::*;
