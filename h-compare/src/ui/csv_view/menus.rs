//! CSV row and column context menus.

use super::*;
use crate::csvdiff::column_backups_excluding;
use std::path::PathBuf;

/// Split the columns into the visible group (checked, in column order) and the
/// hidden group (unchecked, shown below the visible ones).
pub(super) fn column_visibility_entries(
    compare: &CsvCompare,
    hidden: &BTreeSet<usize>,
) -> (Vec<usize>, Vec<usize>) {
    let mut visible = Vec::with_capacity(compare.column_count);
    let mut hidden_cols = Vec::new();
    for col in 0..compare.column_count {
        if hidden.contains(&col) {
            hidden_cols.push(col);
        } else {
            visible.push(col);
        }
    }
    (visible, hidden_cols)
}

/// Apply one checkbox toggle to the hidden-column set.
pub(super) fn set_column_visible(hidden: &mut BTreeSet<usize>, col: usize, visible: bool) {
    if visible {
        hidden.remove(&col);
    } else {
        hidden.insert(col);
    }
}

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
    let hidden_count = state.hidden_count();
    let visible_count = compare.column_count.saturating_sub(hidden_count);
    ui.menu_button(
        format!("Columns ({visible_count}/{})", compare.column_count),
        |ui| {
            ui.set_min_width(360.0);
            common::small_muted(
                ui,
                "Checked columns are visible. Uncheck one to hide it; hidden columns move to the bottom of the list.",
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("csv_columns_menu")
                .max_height(420.0)
                .show(ui, |ui| {
                    let (visible, hidden) =
                        column_visibility_entries(compare, &state.hidden_columns);
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
            if !state.hidden_columns.is_empty() {
                ui.separator();
                if common::intent_button(
                    ui,
                    "Show all columns",
                    common::ButtonIntent::Secondary,
                )
                .clicked()
                {
                    state.hidden_columns.clear();
                    state.hidden_dirty = true;
                }
            }
        },
    );
}

fn column_visibility_row(ui: &mut Ui, compare: &CsvCompare, state: &mut CsvUiState, col: usize) {
    let mut shown = !state.hidden_columns.contains(&col);
    let changes = compare.column_change_count(col);
    let mut toggled = false;
    ui.horizontal(|ui| {
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
        set_column_visible(&mut state.hidden_columns, col, shown);
        state.hidden_dirty = true;
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
    ui.menu_button("Restore deleted columns", |ui| {
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
                if common::intent_button(ui, &label, common::ButtonIntent::Secondary).clicked() {
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
        if common::intent_button(ui, "Copy left row (TSV)", common::ButtonIntent::Ghost).clicked() {
            ui.ctx().copy_text(compare.left.rows[left].join("\t"));
            ui.close_menu();
        }
    }
    if let Some(right) = diff_row.right {
        if common::intent_button(ui, "Copy right row (TSV)", common::ButtonIntent::Ghost).clicked()
        {
            ui.ctx().copy_text(compare.right.rows[right].join("\t"));
            ui.close_menu();
        }
    }
    if diff_row.left.is_some()
        && diff_row.right.is_some()
        && common::intent_button(ui, "Copy row numbers", common::ButtonIntent::Ghost).clicked()
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
    let mut hidden = state.hidden_columns.contains(&col);
    if ui.checkbox(&mut hidden, "Hide column").changed() {
        if hidden {
            state.hidden_columns.insert(col);
        } else {
            state.hidden_columns.remove(&col);
        }
        state.hidden_dirty = true;
        ui.close_menu();
    }
    ui.separator();
    if common::intent_button(ui, "Use as key column", common::ButtonIntent::Ghost).clicked() {
        state.pending_key = Some(col);
        ui.close_menu();
    }
    if common::intent_button(ui, "Copy column name", common::ButtonIntent::Ghost).clicked() {
        ui.ctx().copy_text(compare.column_name(col));
        ui.close_menu();
    }
    if common::intent_button(
        ui,
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
            common::intent_button(
                ui,
                "Delete left column from file…",
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
            common::intent_button(
                ui,
                "Delete right column from file…",
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
            common::intent_button(
                ui,
                "Delete column from both files…",
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
