//! Text and PDF comparison view with side-by-side and merged layouts.

use crate::actions::{ExportKind, UiAction};
use crate::load::{OpenData, OpenKind};
use crate::model::CompareOptions;
use crate::textdiff::{find_ranges, RowKind, TextDiffView};
use crate::ui::common::{self, SearchUiState, SearchUpdate};
use crate::ui::icons::Icon;
use crate::ui::theme;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Label, RichText, Sense, Stroke, TextWrapMode, Ui,
};
use egui_extras::{Column, TableBuilder};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextViewMode {
    Merged,
    SideBySide,
}

#[derive(Debug)]
pub struct TextUiState {
    pub mode: TextViewMode,
    pub selected_row: Option<usize>,
    pub selected_group: usize,
    pub scroll_to_row: Option<usize>,
    pub scroll_to_char: Option<usize>,
    pub current_change: Option<usize>,
    pub search: SearchUiState,
    pub left_width_chars: usize,
    pub right_width_chars: usize,
    pub sync_scroll_x: f32,
    pub sync_scroll_y: f32,
    pub options_collapsed: bool,
}

impl TextUiState {
    pub fn new(view: &TextDiffView) -> Self {
        let first_change = view.change_rows.first().copied();
        let first_group_char = first_change.and_then(|row| group_start_char(view, row, 0));
        Self {
            mode: TextViewMode::SideBySide,
            selected_row: first_change.or_else(|| (!view.rows.is_empty()).then_some(0)),
            selected_group: 0,
            scroll_to_row: first_change,
            scroll_to_char: first_group_char,
            current_change: first_change,
            search: SearchUiState::default(),
            left_width_chars: max_chars(&view.left_lines),
            right_width_chars: max_chars(&view.right_lines),
            sync_scroll_x: 0.0,
            sync_scroll_y: 0.0,
            options_collapsed: false,
        }
    }

    pub fn refresh_search(&mut self, view: &TextDiffView) {
        let query = self.search.query.clone();
        if query.is_empty() {
            self.search.set_matches(Vec::new(), 0);
            return;
        }
        let matches = view.matching_rows(&query, self.search.case_sensitive);
        let count = view.count_matches(&query, self.search.case_sensitive);
        if let Some(row) = self.search.set_matches(matches, count) {
            self.selected_row = Some(row);
            self.selected_group = 0;
            self.scroll_to_row = Some(row);
            self.scroll_to_char = None;
        }
    }

    pub fn goto_change(&mut self, view: &TextDiffView, forward: bool) -> bool {
        let current_row = self.current_change.or(self.selected_row);

        // Step through each contiguous intra-line change group first.
        if let Some(row) = current_row {
            if let Some(diff_row) = view.rows.get(row) {
                let groups = diff_row.change_groups.max(1);
                if groups > 1 {
                    if forward && self.selected_group + 1 < groups {
                        self.selected_group += 1;
                        self.select_group(view, row);
                        return true;
                    }
                    if !forward && self.selected_group > 0 {
                        self.selected_group -= 1;
                        self.select_group(view, row);
                        return true;
                    }
                }
            }
        }

        let target = if forward {
            view.next_change(current_row)
                .or_else(|| view.next_change(None))
        } else {
            view.previous_change(current_row)
                .or_else(|| view.previous_change(None))
        };
        if let Some(row) = target {
            let groups = view
                .rows
                .get(row)
                .map(|row| row.change_groups.max(1))
                .unwrap_or(1);
            self.selected_group = if forward { 0 } else { groups - 1 };
            self.current_change = Some(row);
            self.selected_row = Some(row);
            self.scroll_to_row = Some(row);
            self.scroll_to_char = group_start_char(view, row, self.selected_group);
            true
        } else {
            false
        }
    }

    fn select_group(&mut self, view: &TextDiffView, row: usize) {
        self.current_change = Some(row);
        self.selected_row = Some(row);
        // The row is already visible; do not re-trigger a vertical animation.
        self.scroll_to_char = group_start_char(view, row, self.selected_group);
    }

    /// Move the selection with the keyboard. Up/Down/Page/Home/End move by
    /// line; Left/Right walk the intra-line change groups (the visible cursor)
    /// and advance to the neighbouring change row at a group boundary.
    pub fn navigate(&mut self, view: &TextDiffView, key: common::NavKey) -> bool {
        match key {
            common::NavKey::Left => return self.goto_change(view, false),
            common::NavKey::Right => return self.goto_change(view, true),
            _ => {}
        }
        if view.rows.is_empty() {
            return false;
        }
        let current = self
            .selected_row
            .or(self.current_change)
            .unwrap_or(0)
            .min(view.rows.len() - 1);
        const PAGE_STEP: usize = 10;
        let target = match key {
            common::NavKey::Up => current.saturating_sub(1),
            common::NavKey::Down => (current + 1).min(view.rows.len() - 1),
            common::NavKey::PageUp => current.saturating_sub(PAGE_STEP),
            common::NavKey::PageDown => (current + PAGE_STEP).min(view.rows.len() - 1),
            common::NavKey::Home => 0,
            common::NavKey::End => view.rows.len() - 1,
            common::NavKey::Left | common::NavKey::Right => unreachable!(),
        };
        if target == current && self.selected_row == Some(target) {
            return false;
        }
        self.selected_group = 0;
        self.current_change = if view.rows[target].kind == RowKind::Equal {
            None
        } else {
            Some(target)
        };
        self.selected_row = Some(target);
        self.scroll_to_row = Some(target);
        self.scroll_to_char = None;
        true
    }
}

fn group_start_char(view: &TextDiffView, row: usize, group: usize) -> Option<usize> {
    let diff_row = view.rows.get(row)?;
    diff_row
        .group_left
        .get(group)
        .and_then(|range| range.as_ref())
        .map(|range| range.start)
        .or_else(|| {
            diff_row
                .group_right
                .get(group)
                .and_then(|range| range.as_ref())
                .map(|range| range.start)
        })
}

/// Position of the focused intra-line change group and the total number of
/// change groups. Added/removed lines count as one group.
fn change_progress(view: &TextDiffView, state: &TextUiState) -> (usize, usize) {
    let current_row = state.current_change.or(state.selected_row);
    let mut total = 0usize;
    let mut current = 0usize;
    for (idx, row) in view.rows.iter().enumerate() {
        if matches!(row.kind, RowKind::Equal) {
            continue;
        }
        let count = row.change_groups.max(1);
        if Some(idx) == current_row {
            current = total + state.selected_group.min(count - 1) + 1;
        }
        total += count;
    }
    if total == 0 {
        (0, 0)
    } else {
        (current.max(1), total)
    }
}

fn max_chars(lines: &[String]) -> usize {
    lines
        .iter()
        .map(|line| line.width())
        .max()
        .unwrap_or(0)
        .min(1200)
}

/// Draw the whole text comparison view. Returns true when options changed and
/// the comparison should be rebuilt.
pub fn show(
    ui: &mut Ui,
    view: &TextDiffView,
    state: &mut TextUiState,
    data: &OpenData,
    options: &mut CompareOptions,
    actions: &mut Vec<UiAction>,
) -> bool {
    let mut reload = false;
    let left_name = file_name(data.left_path.as_deref()).unwrap_or_else(|| "(missing)".to_string());
    let right_name =
        file_name(data.right_path.as_deref()).unwrap_or_else(|| "(missing)".to_string());

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(data.rel.as_str()).strong().size(14.0));
        ui.label(
            RichText::new(if data.kind == OpenKind::Pdf {
                "PDF"
            } else if data.kind == OpenKind::Word {
                "WORD"
            } else {
                "TEXT"
            })
            .color(theme::accent())
            .size(10.0)
            .strong(),
        );
        ui.separator();
        common::stat_chip(
            ui,
            "modified",
            view.stats.modified.to_string(),
            theme::diff(),
        );
        common::stat_chip(ui, "added", view.stats.added.to_string(), theme::right());
        common::stat_chip(ui, "removed", view.stats.removed.to_string(), theme::left());
        common::stat_chip(ui, "hunks", view.stats.hunks.to_string(), theme::muted());
        ui.separator();
        let (current, total) = change_progress(view, state);
        if let Some(forward) = common::change_nav(ui, current, total) {
            if !state.goto_change(view, forward) {
                actions.push(UiAction::NextFileDiff { forward });
            }
            ui.ctx().request_repaint();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            common::collapse_arrow(ui, &mut state.options_collapsed);
        });
    });

    if !state.options_collapsed {
        egui::Grid::new("text_toolbar_grid")
            .num_columns(5)
            .spacing(egui::vec2(14.0, 6.0))
            .show(ui, |ui| {
                let mut mode = state.mode;
                common::view_menu(
                    ui,
                    &mut mode,
                    &[
                        (TextViewMode::Merged, "Merged"),
                        (TextViewMode::SideBySide, "Side by side"),
                    ],
                );
                state.mode = mode;
                if ui
                    .checkbox(&mut options.ignore_case, "Ignore case")
                    .on_hover_text("Case-insensitive line comparison")
                    .changed()
                {
                    reload = true;
                }
                if data.left_path.is_some() {
                    if common::icon_button(ui, Icon::Copy, "A → B", common::ButtonIntent::Secondary)
                        .on_hover_text("Copy the left file over the right file")
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
                if data.left_path.is_some() {
                    if common::icon_button(
                        ui,
                        Icon::External,
                        "Open A",
                        common::ButtonIntent::Ghost,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::OpenDefault {
                            rel: data.rel.clone(),
                            left: true,
                        });
                    }
                } else {
                    ui.label("");
                }
                common::find_button(ui, &mut state.search);
                ui.end_row();

                ui.label("");
                if ui
                    .checkbox(&mut options.ignore_whitespace, "Ignore whitespace")
                    .on_hover_text("Collapse runs of whitespace and trim line edges")
                    .changed()
                {
                    reload = true;
                }
                if data.right_path.is_some() {
                    if common::icon_button(ui, Icon::Copy, "B → A", common::ButtonIntent::Secondary)
                        .on_hover_text("Copy the right file over the left file")
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
                if data.right_path.is_some() {
                    if common::icon_button(
                        ui,
                        Icon::External,
                        "Open B",
                        common::ButtonIntent::Ghost,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::OpenDefault {
                            rel: data.rel.clone(),
                            left: false,
                        });
                    }
                } else {
                    ui.label("");
                }
                ui.label("");
                ui.end_row();

                ui.label("");
                if ui
                    .checkbox(&mut options.trim_trailing, "Trim trailing")
                    .on_hover_text("Ignore trailing spaces and tabs")
                    .changed()
                {
                    reload = true;
                }
                let _ = common::icon_menu_button(ui, Icon::Download, "Export", |ui| {
                    if common::icon_button(
                        ui,
                        Icon::Download,
                        "HTML report…",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::TextHtml));
                        common::close_menu(ui);
                    }
                    if common::icon_button(
                        ui,
                        Icon::Download,
                        "Unified diff…",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::TextUnified));
                        common::close_menu(ui);
                    }
                });
                ui.label("");
                ui.label("");
                ui.end_row();
            });

        let encoding_line = format!(
            "A  {} · {} · {} lines   |   B  {} · {} · {} lines",
            view.left_encoding,
            view.left_line_ending.label(),
            view.stats.left_lines,
            view.right_encoding,
            view.right_line_ending.label(),
            view.stats.right_lines
        );
        common::small_muted(
            ui,
            format!(
                "{encoding_line}   ·   Arrow keys move the cursor · F7 / Shift+F7 changes · Ctrl+C copies the selected line"
            ),
        );
    }
    if state.search.open {
        let change = common::search_bar(ui, &mut state.search);
        if change == SearchUpdate::Changed || change == SearchUpdate::Navigate {
            state.refresh_search(view);
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

    ui.add_space(2.0);
    ui.separator();

    if view.rows.is_empty() {
        ui.centered_and_justified(|ui| {
            common::small_muted(ui, "Both files are empty.");
        });
        return reload;
    }

    if let Some(key) = common::take_navigation_key(ui) {
        if state.navigate(view, key) {
            ui.ctx().request_repaint();
        }
    }

    let scroll_to = state.scroll_to_row.take();
    let query = state.search.query.clone();
    let case_sensitive = state.search.case_sensitive;
    let selected_row = state.selected_row;
    let selected_group = state.selected_group;

    common::table_surface(ui);
    let table_ctx = TextTableContext {
        view,
        selected_row,
        selected_group,
        scroll_row: scroll_to,
        query: &query,
        case_sensitive,
    };
    if state.mode == TextViewMode::Merged {
        merged_text_table(ui, &table_ctx, &left_name, &right_name, state);
    } else {
        let glyph = ui.fonts(|fonts| fonts.glyph_width(&FontId::monospace(12.5), 'M'));
        let gutter = 48.0;
        let left_text_w = ((state.left_width_chars as f32 * glyph) + 24.0).clamp(180.0, 6000.0);
        let right_text_w = ((state.right_width_chars as f32 * glyph) + 24.0).clamp(180.0, 6000.0);
        if let Some(ch) = state.scroll_to_char.take() {
            let viewport = (ui.available_width() * 0.5).max(140.0);
            state.sync_scroll_x = common::char_scroll_target(ch, gutter, glyph, viewport);
        }
        if let Some(row) = scroll_to {
            let viewport = (ui.available_height() - 24.0).max(40.0);
            state.sync_scroll_y =
                common::row_scroll_target(row, 18.0, ui.spacing().item_spacing.y, viewport);
        }
        let full_rect = ui.available_rect_before_wrap();
        let pane_height = (ui.available_height() - 32.0).max(80.0);
        let mut left_rect = egui::Rect::NOTHING;
        let mut right_rect = egui::Rect::NOTHING;
        let mut left_offset = egui::Vec2::new(state.sync_scroll_x, state.sync_scroll_y);
        let mut right_offset = egui::Vec2::new(state.sync_scroll_x, state.sync_scroll_y);
        let sync = egui::Vec2::new(state.sync_scroll_x, state.sync_scroll_y);
        let left_pane = PaneLayout {
            name: &left_name,
            right_side: false,
            gutter,
            text_width: left_text_w,
            sync,
        };
        let right_pane = PaneLayout {
            name: &right_name,
            right_side: true,
            gutter,
            text_width: right_text_w,
            sync,
        };

        let pane = ui.allocate_ui(egui::vec2(full_rect.width(), pane_height), |ui| {
            ui.columns(2, |panes| {
                left_rect = panes[0].available_rect_before_wrap();
                left_offset = render_text_pane(&mut panes[0], &table_ctx, &left_pane, state);
                right_rect = panes[1].available_rect_before_wrap();
                right_offset = render_text_pane(&mut panes[1], &table_ctx, &right_pane, state);
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

        let painter = ui.painter();
        painter.vline(
            pane_rect.center().x,
            pane_rect.y_range(),
            Stroke::new(1.0_f32, theme::border_light()),
        );
        for offset in [-5.0_f32, 0.0, 5.0] {
            painter.circle_filled(
                egui::pos2(pane_rect.center().x, pane_rect.top() + 12.0 + offset),
                1.5,
                theme::muted(),
            );
        }

        let viewport = (pane_rect.width() * 0.5).max(140.0);
        let content = gutter + left_text_w.max(right_text_w) + 12.0;
        if let Some(offset) =
            common::horizontal_scroll_bar(ui, state.sync_scroll_x, viewport, content)
        {
            state.sync_scroll_x = offset;
        }
    }

    ui.add_space(4.0);
    common::small_muted(
        ui,
        if state.mode == TextViewMode::Merged {
            "Merged view · Arrow keys move the cursor · F7 / Shift+F7 moves between changes · Ctrl+C copies the selected line"
        } else {
            "Arrow keys move the cursor · Ctrl+C copies the selected line · F7 / Shift+F7 moves between changes · both panes share vertical scroll"
        },
    );
    reload
}

fn file_name(path: Option<&std::path::Path>) -> Option<String> {
    path.and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
}

mod table;
#[cfg(test)]
mod tests;

use table::*;
