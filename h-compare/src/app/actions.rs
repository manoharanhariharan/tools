//! Deferred UI commands, confirmations and file operations.

use super::*;

impl App {
    pub(super) fn process_deferred(&mut self, ctx: &egui::Context) {
        let mut guard = 0;
        while !self.deferred.is_empty() && guard < 128 {
            guard += 1;
            let actions = std::mem::take(&mut self.deferred);
            for action in actions {
                self.apply_action(action, ctx);
            }
        }
    }

    pub(super) fn apply_action(&mut self, action: UiAction, ctx: &egui::Context) {
        match action {
            UiAction::OpenNode(rel) => {
                self.start_open(&rel, ctx);
            }
            UiAction::Copy { rel, from_left } => {
                if let Some((message, kind)) = self.confirm_copy(&rel, from_left) {
                    self.confirm = Some(ConfirmAction { message, kind });
                }
            }
            UiAction::Delete { rel, left } => {
                let Some(tree) = &self.tree else { return };
                let Some(node) = tree.by_rel(&rel) else {
                    return;
                };
                let Some(path) = (if left {
                    node.left_path()
                } else {
                    node.right_path()
                }) else {
                    return;
                };
                self.confirm = Some(ConfirmAction {
                    message: format!(
                        "Delete this file permanently?\n\n{}\n\nThis cannot be undone.",
                        path.display()
                    ),
                    kind: ConfirmKind::Delete { rel, left },
                });
            }
            UiAction::Reveal { rel, left } => {
                if let Some(path) = self.side_path(&rel, left) {
                    reveal_in_explorer(&path);
                }
            }
            UiAction::OpenDefault { rel, left } => {
                if let Some(path) = self.side_path(&rel, left) {
                    open_with_default(&path);
                }
            }
            UiAction::Sync { from_left } => {
                let files = self.sync_candidates(from_left);
                if files.is_empty() {
                    self.set_message("There is nothing to copy in that direction.", false);
                } else {
                    let direction = if from_left {
                        "left → right"
                    } else {
                        "right → left"
                    };
                    self.confirm = Some(ConfirmAction {
                        message: format!(
                            "Copy {} differing file(s) {direction}?\n\nExisting destination files will be overwritten.",
                            files.len()
                        ),
                        kind: ConfirmKind::Sync { from_left, files },
                    });
                }
            }
            UiAction::Export(kind) => self.export(kind, ctx),
            UiAction::Scan => self.start_scan(ctx),
            UiAction::ReloadOpen => self.reload_open(ctx),
            UiAction::NextFileDiff { forward } => self.next_file_diff(forward, ctx),
            UiAction::RecomputeCsv { hidden } => {
                let columns: std::collections::BTreeSet<usize> = hidden.into_iter().collect();
                let ignore = CsvIgnore {
                    left: columns.clone(),
                    right: columns,
                };
                self.recompute_csv_ignore(&ignore);
            }
            UiAction::DeleteCsvColumn { left, column } => {
                let info = self.open.as_ref().and_then(|open| {
                    let csv = open.data.csv.as_ref()?;
                    let (path, table) = if left {
                        (open.data.left_path.as_ref()?, &csv.left)
                    } else {
                        (open.data.right_path.as_ref()?, &csv.right)
                    };
                    if column >= table.column_count {
                        return None;
                    }
                    Some((
                        path.to_path_buf(),
                        column,
                        table.delimiter,
                        table.display_header(column),
                    ))
                });
                if let Some((path, column, delimiter, name)) = info {
                    let side = if left { "left" } else { "right" };
                    self.confirm = Some(ConfirmAction {
                        message: format!(
                            "Delete column {} ({}) from the {side} CSV file?\n\n{}\n\nA timestamped .hcompare.bak backup is created before the file is rewritten.",
                            column + 1,
                            name,
                            path.display()
                        ),
                        kind: ConfirmKind::DeleteCsvColumn {
                            path,
                            column,
                            delimiter,
                        },
                    });
                } else {
                    self.set_message("That column is not available on the selected side.", true);
                }
            }
            UiAction::DeleteCsvColumnBoth { column } => {
                let info = self.open.as_ref().map(|open| {
                    let csv = open.data.csv.as_ref();
                    let mut targets: Vec<(PathBuf, u8)> = Vec::new();
                    let mut names: Vec<String> = Vec::new();
                    if let (Some(csv), Some(path)) = (csv, open.data.left_path.as_ref()) {
                        if column < csv.left.column_count {
                            targets.push((path.to_path_buf(), csv.left.delimiter));
                            names.push(format!("left: {}", csv.left.display_header(column)));
                        }
                    }
                    if let (Some(csv), Some(path)) = (csv, open.data.right_path.as_ref()) {
                        if column < csv.right.column_count {
                            targets.push((path.to_path_buf(), csv.right.delimiter));
                            names.push(format!("right: {}", csv.right.display_header(column)));
                        }
                    }
                    (targets, names)
                });
                if let Some((targets, names)) = info {
                    if targets.is_empty() {
                        self.set_message("That column is not available on either side.", true);
                    } else {
                        let file_list = targets
                            .iter()
                            .map(|(path, _)| format!("  • {}", path.display()))
                            .collect::<Vec<_>>()
                            .join("\n");
                        self.confirm = Some(ConfirmAction {
                            message: format!(
                                "Delete column {} ({}) from both CSV files?\n\n{file_list}\n\nA timestamped .hcompare.bak backup is created for each rewritten file.",
                                column + 1,
                                names.join(" · "),
                            ),
                            kind: ConfirmKind::DeleteCsvColumnBoth { targets, column },
                        });
                    }
                } else {
                    self.set_message("No CSV file is open.", true);
                }
            }
            UiAction::RestoreCsvBackup { path, backup } => {
                if self.csv_restored_backups.contains(&backup) {
                    self.set_message("That backup was already restored.", false);
                    return;
                }
                match restore_column_backup(&path, &backup) {
                    Ok(()) => {
                        self.csv_restored_backups.insert(backup);
                        self.csv_ignores.clear();
                        self.csv_column_undo = None;
                        self.set_message(
                            format!("Restored {} from backup.", path.display()),
                            false,
                        );
                        self.start_scan(ctx);
                    }
                    Err(error) => self.set_message(format!("Restore failed: {error}"), true),
                }
            }
            UiAction::UndoCsvColumnDelete => {
                let Some(undo) = self.csv_column_undo.take() else {
                    self.set_message("Nothing to undo.", true);
                    return;
                };
                let mut errors = Vec::new();
                let mut restored = Vec::new();
                for entry in &undo.entries {
                    match restore_column_backup(&entry.path, &entry.backup) {
                        Ok(()) => restored.push(entry.backup.clone()),
                        Err(error) => errors.push(format!("{}: {error}", entry.path.display())),
                    }
                }
                for backup in restored {
                    self.csv_restored_backups.insert(backup);
                }
                if errors.is_empty() {
                    self.csv_ignores.clear();
                    self.set_message(
                        format!(
                            "Restored deleted column {} in {} file(s).",
                            undo.column + 1,
                            undo.entries.len()
                        ),
                        false,
                    );
                    self.start_scan(ctx);
                } else {
                    self.csv_column_undo = Some(undo);
                    self.set_message(
                        format!("Undo failed for {}: {}", errors.len(), errors[0]),
                        true,
                    );
                }
            }
        }
    }

    pub(super) fn sync_csv_view_state(&mut self) {
        let Some(csv_ui) = self.open.as_ref().and_then(|open| open.csv_ui.as_ref()) else {
            return;
        };
        let side_by_side = csv_ui.mode == CsvViewMode::SideBySide;
        let differences_only = csv_ui.differences_only;
        let options_collapsed = csv_ui.options_collapsed;
        if self.settings.csv_side_by_side != side_by_side
            || self.settings.csv_differences_only != differences_only
            || self.settings.csv_options_collapsed != options_collapsed
        {
            self.settings.csv_side_by_side = side_by_side;
            self.settings.csv_differences_only = differences_only;
            self.settings.csv_options_collapsed = options_collapsed;
            let _ = self.settings.save();
        }
    }

    pub(super) fn sync_text_view_state(&mut self) {
        let Some(text_ui) = self.open.as_ref().and_then(|open| open.text_ui.as_ref()) else {
            return;
        };
        if self.settings.text_options_collapsed != text_ui.options_collapsed {
            self.settings.text_options_collapsed = text_ui.options_collapsed;
            let _ = self.settings.save();
        }
    }

    pub(super) fn recompute_csv_ignore(&mut self, ignore: &CsvIgnore) {
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(csv) = open.data.csv.as_ref() else {
            return;
        };
        let rel = open.data.rel.clone();
        let new_compare = compare_tables_with_ignore(
            csv.left.clone(),
            csv.right.clone(),
            &self.settings.csv,
            ignore,
        );
        if let Some(csv_ui) = open.csv_ui.as_mut() {
            let selected = csv_ui.selected_row;
            let current = csv_ui.current_change;
            csv_ui.set_hidden(ignore);
            csv_ui.ensure_widths(&new_compare);
            csv_ui.selected_row = selected.filter(|row| *row < new_compare.rows.len());
            csv_ui.current_change = current.filter(|row| *row < new_compare.rows.len());
            csv_ui.refresh_search(&new_compare);
        }
        open.data.csv = Some(new_compare);
        self.csv_ignores.insert(rel, ignore.clone());
    }

    pub(super) fn side_path(&self, rel: &str, left: bool) -> Option<PathBuf> {
        let node = self.tree.as_ref()?.by_rel(rel)?;
        if left {
            node.left_path().map(Path::to_path_buf)
        } else {
            node.right_path().map(Path::to_path_buf)
        }
    }

    pub(super) fn confirm_copy(&self, rel: &str, from_left: bool) -> Option<(String, ConfirmKind)> {
        let tree = self.tree.as_ref()?;
        let node = tree.by_rel(rel)?;
        let source = if from_left {
            node.left_path()?
        } else {
            node.right_path()?
        };
        let target = if from_left {
            node.right_path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| tree.right_root.join(rel))
        } else {
            node.left_path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| tree.left_root.join(rel))
        };
        let verb = if target.exists() {
            "Overwrite this file?"
        } else {
            "Create this file?"
        };
        Some((
            format!(
                "{verb}\n\nFrom: {}\nTo: {}\n\nExisting destination files will be overwritten.",
                source.display(),
                target.display()
            ),
            ConfirmKind::Copy {
                rel: rel.to_string(),
                from_left,
            },
        ))
    }

    pub(super) fn sync_candidates(&self, from_left: bool) -> Vec<String> {
        let Some(tree) = &self.tree else {
            return Vec::new();
        };
        tree.files(tree.root)
            .into_iter()
            .filter_map(|idx| {
                let node = &tree.nodes[idx];
                if node.is_dir || node.status == EntryStatus::Same {
                    return None;
                }
                let source = if from_left {
                    node.left.as_ref()
                } else {
                    node.right.as_ref()
                }?;
                if source.is_dir {
                    return None;
                }
                let target = if from_left {
                    node.right.as_ref()
                } else {
                    node.left.as_ref()
                };
                if target.map(|entry| entry.is_dir).unwrap_or(false) {
                    return None;
                }
                if from_left && node.right.is_none() {
                    return Some(node.rel.clone());
                }
                if !from_left && node.left.is_none() {
                    return Some(node.rel.clone());
                }
                if matches!(
                    node.status,
                    EntryStatus::Different | EntryStatus::LeftOnly | EntryStatus::RightOnly
                ) {
                    Some(node.rel.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) fn execute_confirm(&mut self, action: ConfirmAction, ctx: &egui::Context) {
        match action.kind {
            ConfirmKind::Copy { rel, from_left } => match self.copy_one(&rel, from_left) {
                Ok(()) => {
                    self.set_message("File copied.", false);
                    self.start_scan(ctx);
                }
                Err(error) => self.set_message(format!("Copy failed: {error}"), true),
            },
            ConfirmKind::Delete { rel, left } => {
                if let Some(path) = self.side_path(&rel, left) {
                    match fs::remove_file(&path) {
                        Ok(()) => {
                            self.set_message("File deleted.", false);
                            self.start_scan(ctx);
                        }
                        Err(error) => self.set_message(format!("Delete failed: {error}"), true),
                    }
                }
            }
            ConfirmKind::DeleteCsvColumn {
                path,
                column,
                delimiter,
            } => match delete_column_from_file(&path, column, delimiter) {
                Ok(backup) => {
                    self.csv_restored_backups.remove(&backup);
                    self.csv_ignores.clear();
                    self.csv_column_undo = Some(CsvColumnUndo {
                        column,
                        entries: vec![CsvColumnUndoEntry {
                            path: path.clone(),
                            backup: backup.clone(),
                        }],
                    });
                    self.set_message(
                        format!(
                            "Deleted column {}. Backup: {}",
                            column + 1,
                            backup.display()
                        ),
                        false,
                    );
                    self.start_scan(ctx);
                }
                Err(error) => self.set_message(format!("Delete column failed: {error}"), true),
            },
            ConfirmKind::DeleteCsvColumnBoth { targets, column } => {
                let mut entries = Vec::new();
                let mut errors = Vec::new();
                for (path, delimiter) in targets {
                    match delete_column_from_file(&path, column, delimiter) {
                        Ok(backup) => {
                            self.csv_restored_backups.remove(&backup);
                            entries.push(CsvColumnUndoEntry { path, backup });
                        }
                        Err(error) => errors.push(format!("{}: {error}", path.display())),
                    }
                }
                if entries.is_empty() {
                    self.set_message(
                        format!(
                            "Delete column failed: {}",
                            errors.first().cloned().unwrap_or_default()
                        ),
                        true,
                    );
                } else {
                    self.csv_ignores.clear();
                    self.csv_column_undo = Some(CsvColumnUndo { column, entries });
                    let done = if errors.is_empty() {
                        format!("Deleted column {} from both files.", column + 1)
                    } else {
                        format!(
                            "Deleted column {} from {} file(s). First error: {}",
                            column + 1,
                            errors.len().max(1),
                            errors[0]
                        )
                    };
                    self.set_message(done, !errors.is_empty());
                    self.start_scan(ctx);
                }
            }
            ConfirmKind::Sync { from_left, files } => {
                let total = files.len();
                let mut errors = Vec::new();
                for rel in files {
                    if let Err(error) = self.copy_one(&rel, from_left) {
                        errors.push(format!("{rel}: {error}"));
                    }
                }
                if errors.is_empty() {
                    self.set_message(format!("Copied {total} file(s)."), false);
                } else {
                    self.set_message(
                        format!(
                            "Copied {} of {total} file(s). First error: {}",
                            total - errors.len(),
                            errors[0]
                        ),
                        true,
                    );
                }
                self.start_scan(ctx);
            }
        }
    }

    pub(super) fn copy_one(&self, rel: &str, from_left: bool) -> Result<(), String> {
        let tree = self.tree.as_ref().ok_or("No comparison loaded")?;
        let node = tree
            .by_rel(rel)
            .ok_or("File is no longer in the comparison")?;
        let source = if from_left {
            node.left_path()
        } else {
            node.right_path()
        }
        .ok_or("Source file is missing")?;
        let target = if from_left {
            node.right_path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| tree.right_root.join(rel))
        } else {
            node.left_path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| tree.left_root.join(rel))
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(source, target)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub(super) fn export(&mut self, kind: ExportKind, ctx: &egui::Context) {
        let outcome: Option<Result<(PathBuf, String), String>> = {
            let Some(open) = &self.open else {
                return;
            };
            let rel = open.data.rel.clone();
            let left_name = open
                .data
                .left_path
                .as_deref()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "left".to_string());
            let right_name = open
                .data
                .right_path
                .as_deref()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "right".to_string());
            let (default_name, filter_name, filter_ext) = match kind {
                ExportKind::CsvDifferences => ("differences.csv", "CSV", "csv"),
                ExportKind::CsvHtml | ExportKind::TextHtml => ("comparison.html", "HTML", "html"),
                ExportKind::TextUnified => ("changes.diff", "Diff", "diff"),
            };
            let Some(path) = rfd::FileDialog::new()
                .set_title(format!("Export {rel}"))
                .set_file_name(default_name)
                .add_filter(filter_name, &[filter_ext])
                .save_file()
            else {
                return;
            };
            Some(match kind {
                ExportKind::CsvDifferences => open
                    .data
                    .csv
                    .as_ref()
                    .map(report::csv_differences)
                    .map(|content| (path, content))
                    .ok_or_else(|| "Current file is not a CSV comparison.".to_string()),
                ExportKind::CsvHtml => open
                    .data
                    .csv
                    .as_ref()
                    .map(|compare| {
                        (
                            path,
                            report::csv_html_report(compare, &left_name, &right_name),
                        )
                    })
                    .ok_or_else(|| "Current file is not a CSV comparison.".to_string()),
                ExportKind::TextHtml => open
                    .data
                    .text
                    .as_ref()
                    .map(|view| {
                        (
                            path,
                            report::text_html_report(view, &left_name, &right_name),
                        )
                    })
                    .ok_or_else(|| "Current file is not a text comparison.".to_string()),
                ExportKind::TextUnified => open
                    .data
                    .text
                    .as_ref()
                    .map(|view| {
                        let left = view.left_lines.join("\n");
                        let right = view.right_lines.join("\n");
                        (
                            path,
                            textdiff::unified_diff(&left, &right, &left_name, &right_name),
                        )
                    })
                    .ok_or_else(|| "Current file is not a text comparison.".to_string()),
            })
        };

        match outcome {
            Some(Ok((path, content))) => match fs::write(&path, content) {
                Ok(()) => self.set_message(format!("Exported {}.", path.display()), false),
                Err(error) => self.set_message(format!("Export failed: {error}"), true),
            },
            Some(Err(error)) => self.set_message(error, true),
            None => {}
        }
        ctx.request_repaint();
    }
}
