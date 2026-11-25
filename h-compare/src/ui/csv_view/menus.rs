//! CSV row and column context menus.

use super::*;
use crate::csvdiff::column_backups_excluding;
use crate::ui::icons::Icon;
use std::path::PathBuf;

/// Tooltip describing why a column header is highlighted.
pub(super) fn column_header_tooltip(compare: &CsvCompare, col: usize) -> Option<String> {
    let changes = compare.column_change_count(col);
    let header_changed = compare.changed_headers.contains(&col);
    if changes == 0 && !header_changed {
        return None;
    }
    let mut lines = Vec::new();
    if header_changed {
        lines.push(format!(
            "Header differs\nLeft: {}\nRight: {}",
            compare.left.display_header(col),
            compare.right.display_header(col)
        ));
    }
    if changes > 0 {
        lines.push(format!(
            "{changes} differing cell{} in this column",
            if changes == 1 { "" } else { "s" }
        ));
    }
    Some(lines.join("\n\n"))
}

/// Toolbar column picker: every column is listed and checked while visible.
/// Unchecking one hides it and moves it into the hidden group at the bottom.
pub(super) fn columns_menu(ui: &mut Ui, compare: &CsvCompare, state: &mut CsvUiState) {
    let (visible_entries, hidden_entries) = state.column_visibility_entries(compare);
    let visible_count = visible_entries.len();
    let _ = common::icon_menu_button(
        ui,
        Icon::Columns,
        &format!("Columns ({visible_count}/{})", compare.column_count),
        |ui| {
            let scroll = &mut ui.style_mut().spacing.scroll;
            scroll.floating = false;
            scroll.bar_width = 18.0;
            scroll.bar_inner_margin = 4.0;
            scroll.bar_outer_margin = 2.0;
            scroll.handle_min_length = 48.0;
            let screen = ui.ctx().screen_rect();
            let width = (screen.width() - 80.0).clamp(360.0, 560.0);
            let max_height = (screen.height() - 180.0).clamp(240.0, 560.0);
            ui.set_min_width(width);
            common::small_muted(
                ui,
                if state.differences_only {
                    "Differences only is on, so unchanged columns are hidden automatically. Check one to show it; uncheck to hide it again."
                } else {
                    "Checked columns are visible. Uncheck one to hide it; hidden columns move to the bottom of the list."
                },
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("csv_columns_menu")
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (visible, hidden) = state.column_visibility_entries(compare);
                    for col in visible {
                        column_visibility_row(ui, compare, state, col);
                    }
                    if !hidden.is_empty() {
                        ui.add_space(6.0);
                        ui.separator();
                        common::section_heading(ui, format!("Hidden columns ({})", hidden.len()));
                        for col in hidden {
                            column_visibility_row(ui, compare, state, col);
                        }
                    }
                });
            if !state.hidden_columns.is_empty() || !hidden_entries.is_empty() {
                ui.separator();
                if common::icon_button(
                    ui,
                    Icon::Check,
                    "Show all columns",
                    common::ButtonIntent::Secondary,
                )
                .clicked()
                {
                    let had_hidden = !state.hidden_columns.is_empty();
                    state.hidden_columns.clear();
                    state.shown_columns = (0..compare.column_count).collect();
                    state.hidden_dirty = had_hidden;
                    common::close_menu(ui);
                }
            }
        },
    );
}

fn column_visibility_row(ui: &mut Ui, compare: &CsvCompare, state: &mut CsvUiState, col: usize) {
    let mut shown = state.column_visible(compare, col);
    let changes = compare.column_change_count(col);
    let mut toggled = false;
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        toggled |= ui.checkbox(&mut shown, "").changed();
        let mut text = RichText::new(format!("{}  {}", col + 1, compare.column_name(col)));
        if changes > 0 {
            text = text.color(theme::diff());
        }
        let response = ui.add(Label::new(text).truncate().sense(Sense::click()));
        let clicked = response.clicked();
        if let Some(tip) = column_header_tooltip(compare, col) {
            response.on_hover_ui(|ui| {
                ui.set_max_width(420.0);
                ui.label(RichText::new(tip).monospace().size(12.0));
            });
        }
        if clicked {
            shown = !shown;
            toggled = true;
        }
        if changes > 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                common::small_muted(
                    ui,
                    format!("{changes} diff{}", if changes == 1 { "" } else { "s" }),
                );
            });
        }
    });
    if toggled {
        state.set_column_visible(compare, col, shown);
        state.refresh_search(compare);
    }
}

/// List the `.hcompare.bak` backups beside the open CSVs. Backups restored in
/// this session are omitted so the same restore is not offered again.
pub(super) fn restore_columns_menu(
    ui: &mut Ui,
    data: &OpenData,
    restored_backups: &BTreeSet<PathBuf>,
    actions: &mut Vec<UiAction>,
) {
    let _ = common::icon_menu_button(ui, Icon::Restore, "Restore deleted", |ui| {
        ui.set_min_width(360.0);
        let mut any = false;
        for (side, path) in [
            ("Left", data.left_path.as_deref()),
            ("Right", data.right_path.as_deref()),
        ] {
            let Some(path) = path else { continue };
            let backups = column_backups_excluding(path, restored_backups);
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
                if common::icon_button(ui, Icon::Restore, &label, common::ButtonIntent::Secondary)
                    .clicked()
                {
                    actions.push(UiAction::RestoreCsvBackup {
                        path: path.to_path_buf(),
                        backup: backup.clone(),
                    });
                    common::close_menu(ui);
                }
            }
            ui.separator();
        }
        if !any {
            common::small_muted(
                ui,
                "No unrestored .hcompare.bak backups beside these CSV files.",
            );
        }
    });
}

pub(super) fn row_context_menu(ui: &mut Ui, compare: &CsvCompare, row: usize) {
    let Some(diff_row) = compare.rows.get(row) else {
        return;
    };
    if let Some(left) = diff_row.left {
        if common::icon_button(
            ui,
            Icon::Copy,
            "Copy left row (TSV)",
            common::ButtonIntent::Ghost,
        )
        .clicked()
        {
            ui.ctx().copy_text(compare.left.rows[left].join("\t"));
            ui.close_menu();
        }
    }
    if let Some(right) = diff_row.right {
        if common::icon_button(
            ui,
            Icon::Copy,
            "Copy right row (TSV)",
            common::ButtonIntent::Ghost,
        )
        .clicked()
        {
            ui.ctx().copy_text(compare.right.rows[right].join("\t"));
            ui.close_menu();
        }
    }
    if diff_row.left.is_some()
        && diff_row.right.is_some()
        && common::icon_button(
            ui,
            Icon::Copy,
            "Copy row numbers",
            common::ButtonIntent::Ghost,
        )
        .clicked()
    {
        let left = diff_row
            .left
            .map(|line| compare.left.row_number(line).to_string())
            .unwrap_or_default();
        let right = diff_row
            .right
            .map(|line| compare.right.row_number(line).to_string())
            .unwrap_or_default();
        ui.ctx().copy_text(format!("{left}\t{right}"));
        ui.close_menu();
    }
}

pub(super) fn column_context_menu(
    ui: &mut Ui,
    compare: &CsvCompare,
    col: usize,
    state: &mut CsvUiState,
) {
    let mut visible = state.column_visible(compare, col);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), Sense::hover());
        crate::ui::icons::paint(
            ui.painter(),
            rect,
            if visible { Icon::Eye } else { Icon::EyeOff },
            theme::muted(),
        );
        if ui.checkbox(&mut visible, "Visible").changed() {
            state.set_column_visible(compare, col, visible);
            state.refresh_search(compare);
            ui.close_menu();
        }
    });
    ui.separator();
    if common::icon_button(
        ui,
        Icon::Key,
        "Use as key column",
        common::ButtonIntent::Ghost,
    )
    .clicked()
    {
        state.pending_key = Some(col);
        ui.close_menu();
    }
    if common::icon_button(
        ui,
        Icon::Copy,
        "Copy column name",
        common::ButtonIntent::Ghost,
    )
    .clicked()
    {
        ui.ctx().copy_text(compare.column_name(col));
        ui.close_menu();
    }
    if common::icon_button(
        ui,
        Icon::Copy,
        &format!("Copy column {}", col + 1),
        common::ButtonIntent::Ghost,
    )
    .clicked()
    {
        ui.ctx().copy_text((col + 1).to_string());
        ui.close_menu();
    }
    ui.separator();
    let left_enabled = col < compare.left.column_count;
    let left_clicked = ui
        .add_enabled_ui(left_enabled, |ui| {
            common::icon_button(
                ui,
                Icon::Trash,
                "Delete left column…",
                common::ButtonIntent::Danger,
            )
        })
        .inner
        .on_hover_text("Rewrite the left CSV without this column (a backup is created)")
        .clicked();
    if left_clicked {
        state.pending_delete = Some((CsvDeleteTarget::Left, col));
        ui.close_menu();
    }
    let right_enabled = col < compare.right.column_count;
    let right_clicked = ui
        .add_enabled_ui(right_enabled, |ui| {
            common::icon_button(
                ui,
                Icon::Trash,
                "Delete right column…",
                common::ButtonIntent::Danger,
            )
        })
        .inner
        .on_hover_text("Rewrite the right CSV without this column (a backup is created)")
        .clicked();
    if right_clicked {
        state.pending_delete = Some((CsvDeleteTarget::Right, col));
        ui.close_menu();
    }
    let both_enabled = left_enabled || right_enabled;
    let both_clicked = ui
        .add_enabled_ui(both_enabled, |ui| {
            common::icon_button(
                ui,
                Icon::Trash,
                "Delete column from both…",
                common::ButtonIntent::Danger,
            )
        })
        .inner
        .on_hover_text(
            "Rewrite both CSVs without this column (a timestamped backup is created for each)",
        )
        .clicked();
    if both_clicked {
        state.pending_delete = Some((CsvDeleteTarget::Both, col));
        ui.close_menu();
    }
}
