//! CSV row and column context menus.

use super::*;

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
