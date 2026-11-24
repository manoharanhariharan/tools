//! About, options, warnings and confirmation dialogs.

use super::*;

impl App {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        let mut open_settings = self.show_settings;
        if open_settings {
            let mut reload = false;
            let mut rescan = false;
            let mut close = false;
            egui::Window::new("Comparison options")
                .open(&mut open_settings)
                .collapsible(false)
                .resizable(false)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Text comparison").strong());
                    ui.checkbox(&mut self.settings.compare.ignore_case, "Ignore case");
                    ui.checkbox(
                        &mut self.settings.compare.ignore_whitespace,
                        "Ignore all whitespace differences",
                    );
                    ui.checkbox(
                        &mut self.settings.compare.trim_trailing,
                        "Ignore trailing whitespace",
                    );
                    ui.separator();
                    ui.label(RichText::new("CSV / table comparison").strong());
                    ui.checkbox(&mut self.settings.csv.has_headers, "First row is a header");
                    ui.checkbox(
                        &mut self.settings.csv.ignore_case,
                        "Ignore case while matching cells",
                    );
                    ui.checkbox(
                        &mut self.settings.csv.trim_whitespace,
                        "Trim spaces around cell values",
                    );
                    ui.checkbox(
                        &mut self.settings.csv.ignore_empty_rows,
                        "Ignore completely empty rows",
                    );
                    ui.separator();
                    ui.label(RichText::new("Folder scan").strong());
                    if ui
                        .checkbox(
                            &mut self.settings.compare.skip_build_dirs,
                            "Skip common build folders (.git, node_modules, target, dist, …)",
                        )
                        .changed()
                    {
                        rescan = true;
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if common::icon_button(
                            ui,
                            Icon::Refresh,
                            "Reload open file",
                            common::ButtonIntent::Secondary,
                        )
                        .clicked()
                        {
                            reload = true;
                        }
                        if common::icon_button(
                            ui,
                            Icon::Refresh,
                            "Rescan folders",
                            common::ButtonIntent::Secondary,
                        )
                        .clicked()
                        {
                            rescan = true;
                        }
                        if common::icon_button(ui, Icon::Close, "Close", common::ButtonIntent::Ghost)
                            .clicked()
                        {
                            close = true;
                        }
                    });
                    ui.add_space(4.0);
                    common::small_muted(
                        ui,
                        "Options are saved automatically. Theme presets live in the top-bar Settings menu.",
                    );
                });
            if close {
                open_settings = false;
            }
            self.show_settings = open_settings;
            if reload {
                self.deferred.push(UiAction::ReloadOpen);
            }
            if rescan {
                self.start_scan(ctx);
            }
        }

        let mut open_about = self.show_about;
        if open_about {
            egui::Window::new("About H-Compare")
                .open(&mut open_about)
                .collapsible(false)
                .resizable(false)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(format!("H-Compare {}", env!("CARGO_PKG_VERSION")))
                            .size(17.0)
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!(
                            "Build {} · {}",
                            env!("HCOMPARE_BUILD_DATE"),
                            env!("HCOMPARE_GIT_HASH")
                        ))
                        .color(theme::faint())
                        .size(11.0),
                    );
                    ui.label(
                        RichText::new(
                            "Native folder, text and CSV comparison built in Rust with egui.",
                        )
                        .color(theme::muted()),
                    );
                    ui.separator();
                    for (key, action) in [
                        ("F5", "Rescan both folders"),
                        ("F7 / Shift+F7", "Next / previous change"),
                        ("Arrow keys", "Move the CSV cell selection or text/PDF cursor"),
                        ("Home / End", "First / last CSV column or text line"),
                        ("PageUp / PageDown", "Move ten CSV rows or text lines"),
                        ("Ctrl+F", "Find in the current text or CSV comparison"),
                        ("Ctrl+B", "Collapse or expand the folder tree"),
                        ("Ctrl+C", "Copy the selected line, cell or row"),
                        ("Ctrl+Z", "Undo the last deleted CSV column"),
                        ("Escape", "Close search or dialog"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(key).monospace().color(theme::accent()));
                            ui.label(RichText::new(action).color(theme::text()));
                        });
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(
                            "Text files are matched line-by-line; CSV files are matched by row order or by a key column, with cell-level highlighting.",
                        )
                        .color(theme::muted())
                        .size(11.5),
                    );
                });
            self.show_about = open_about;
        }

        let mut open_warnings = self.show_warnings;
        if open_warnings {
            let warnings = self
                .tree
                .as_ref()
                .map(|tree| tree.warnings.clone())
                .unwrap_or_default();
            egui::Window::new("Scan warnings")
                .open(&mut open_warnings)
                .collapsible(false)
                .resizable(true)
                .default_width(650.0)
                .show(ctx, |ui| {
                    if warnings.is_empty() {
                        ui.label("No warnings.");
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .show(ui, |ui| {
                                for warning in &warnings {
                                    ui.label(
                                        RichText::new(warning).color(theme::diff()).size(11.5),
                                    );
                                }
                            });
                    }
                });
            self.show_warnings = open_warnings;
        }

        if self.confirm.is_some() {
            let mut confirmed = false;
            let mut cancelled = false;
            if let Some(confirm) = &self.confirm {
                egui::Window::new("Confirm")
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(470.0)
                    .show(ctx, |ui| {
                        ui.label(RichText::new(&confirm.message).color(theme::text()));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if common::icon_button(
                                ui,
                                Icon::Close,
                                "Cancel",
                                common::ButtonIntent::Ghost,
                            )
                            .clicked()
                            {
                                cancelled = true;
                            }
                            let danger = matches!(
                                confirm.kind,
                                ConfirmKind::Delete { .. } | ConfirmKind::DeleteCsvColumn { .. }
                            );
                            let intent = if danger {
                                common::ButtonIntent::Danger
                            } else {
                                common::ButtonIntent::Primary
                            };
                            let (icon, label) = if danger {
                                (Icon::Trash, "Delete")
                            } else {
                                (Icon::Check, "Continue")
                            };
                            if common::icon_button(ui, icon, label, intent).clicked() {
                                confirmed = true;
                            }
                        });
                    });
            }
            if confirmed {
                if let Some(action) = self.confirm.take() {
                    self.execute_confirm(action, ctx);
                }
            } else if cancelled {
                self.confirm = None;
            }
        }
    }
}
