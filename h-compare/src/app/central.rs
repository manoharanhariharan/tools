//! Central comparison area: welcome screen, folder overview and open file.

use super::*;

impl App {
    pub(super) fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(theme::bg())
                    .inner_margin(Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                if self.tree.is_none() {
                    if self.scanning() {
                        ui.add_space(60.0);
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new("Scanning both folder trees…")
                                    .color(theme::muted())
                                    .size(14.0),
                            );
                        });
                    } else {
                        self.welcome(ui, ctx);
                    }
                    return;
                }

                if let Some(error) = &self.scan_error {
                    Frame::NONE
                        .fill(theme::del_bg())
                        .stroke(Stroke::new(1.0_f32, theme::error().gamma_multiply(0.6)))
                        .corner_radius(6.0)
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new("Scan failed:").color(theme::error()).strong(),
                                );
                                ui.label(RichText::new(error).color(theme::text()));
                                if ui.small_button("Retry").clicked() {
                                    self.deferred.push(UiAction::Scan);
                                }
                            });
                        });
                    ui.add_space(6.0);
                }

                let mut reload = false;
                if let Some(open) = &mut self.open {
                    match open.data.kind {
                        OpenKind::Text | OpenKind::Pdf => {
                            if let (Some(view), Some(text_ui)) =
                                (open.data.text.as_ref(), open.text_ui.as_mut())
                            {
                                reload = ui::text_view::show(
                                    ui,
                                    view,
                                    text_ui,
                                    &open.data,
                                    &mut self.settings.compare,
                                    &mut self.deferred,
                                );
                            }
                        }
                        OpenKind::Csv => {
                            if let (Some(compare), Some(csv_ui)) =
                                (open.data.csv.as_ref(), open.csv_ui.as_mut())
                            {
                                reload = ui::csv_view::show(
                                    ui,
                                    compare,
                                    csv_ui,
                                    &open.data,
                                    &mut self.settings.csv,
                                    &mut self.deferred,
                                    self.csv_column_undo.is_some(),
                                );
                            }
                        }
                        OpenKind::Binary => {
                            binary_panel(ui, &open.data, &mut self.deferred);
                        }
                        OpenKind::Error => {
                            error_panel(ui, &open.data, &mut self.deferred);
                        }
                    }
                }
                if reload {
                    self.deferred.push(UiAction::ReloadOpen);
                }

                if self.open.is_none() {
                    if let Some(pending) = &self.pending_open {
                        ui.add_space(50.0);
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(format!("Loading {}…", pending.rel))
                                    .color(theme::muted()),
                            );
                        });
                    } else if let Some((rel, error)) = &self.open_error {
                        let rel = rel.clone();
                        let error = error.clone();
                        error_card(ui, &rel, &error, &mut self.deferred);
                    } else {
                        self.folder_overview(ui);
                    }
                }
            });
    }

    pub(super) fn welcome(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut recent_pick: Option<(String, String)> = None;
        let recent = self.settings.recent.clone();
        ui.add_space(34.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Compare folders without the noise.")
                    .color(theme::text())
                    .size(27.0)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "A fast, native folder, text and CSV diff tool. Side-by-side trees, line-level highlights, and a table-aware CSV diff.",
                )
                .color(theme::muted())
                .size(13.0),
            );
        });
        ui.add_space(22.0);

        if let Some(error) = &self.scan_error {
            Frame::NONE
                .fill(theme::del_bg())
                .stroke(Stroke::new(1.0_f32, theme::error().gamma_multiply(0.6)))
                .corner_radius(6.0)
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.label(RichText::new(error).color(theme::error()));
                });
            ui.add_space(10.0);
        }

        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() * 0.16);
            ui.vertical(|ui| {
                ui.set_max_width(680.0);
                Frame::NONE
                    .fill(theme::panel())
                    .stroke(Stroke::new(1.0_f32, theme::border()))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(18, 14))
                    .show(ui, |ui| {
                        ui.label(RichText::new("1  Choose the two folders").strong());
                        ui.add_space(4.0);
                        let left = path_field(
                            ui,
                            "A",
                            theme::accent(),
                            &mut self.left_path,
                            "Left folder…",
                        );
                        let right = path_field(
                            ui,
                            "B",
                            theme::right(),
                            &mut self.right_path,
                            "Right folder…",
                        );
                        let compare_enter = (left.enter || right.enter)
                            && !self.left_path.trim().is_empty()
                            && !self.right_path.trim().is_empty();
                        if compare_enter {
                            self.start_scan(ctx);
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if common::accent_button(ui, "Compare folders").clicked() {
                                self.start_scan(ctx);
                            }
                            if common::intent_button(
                                ui,
                                "Browse left…",
                                common::ButtonIntent::Ghost,
                            )
                            .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Choose the left folder")
                                    .pick_folder()
                                {
                                    self.left_path = path.display().to_string();
                                }
                            }
                            if common::intent_button(
                                ui,
                                "Browse right…",
                                common::ButtonIntent::Ghost,
                            )
                            .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Choose the right folder")
                                    .pick_folder()
                                {
                                    self.right_path = path.display().to_string();
                                }
                            }
                        });
                    });
                if !recent.is_empty() {
                    ui.add_space(14.0);
                    Frame::NONE
                        .fill(theme::panel())
                        .stroke(Stroke::new(1.0_f32, theme::border()))
                        .corner_radius(8.0)
                        .inner_margin(Margin::symmetric(18, 12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Recent comparisons").strong());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Clear")
                                                    .color(theme::muted())
                                                    .size(11.0),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("Clear recent comparisons")
                                        .clicked()
                                    {
                                        self.settings.recent.clear();
                                        self.persist();
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            for entry in recent.iter().take(6) {
                                let label = format!(
                                    "{}   ↔   {}",
                                    short_path(&entry.left),
                                    short_path(&entry.right)
                                );
                                if common::card_button(ui, &label, false)
                                    .on_hover_text(format!("{}\n{}", entry.left, entry.right))
                                    .clicked()
                                {
                                    recent_pick = Some((entry.left.clone(), entry.right.clone()));
                                }
                            }
                        });
                }
                ui.add_space(16.0);
                ui.label(RichText::new("What you get").strong());
                ui.add_space(4.0);
                for line in [
                    "A status-aware folder tree with one-click filtering and instant rescan.",
                    "Side-by-side text diff with intra-line highlights, search and F7 change navigation.",
                    "CSV table diff with cell-level highlighting, key alignment, and merged or side-by-side views.",
                    "Safe copy and sync actions for individual files or every difference at once.",
                ] {
                    ui.label(RichText::new(format!("•  {line}")).color(theme::muted()));
                }
                ui.add_space(14.0);
                common::small_muted(
                    ui,
                    "Shortcuts: F5 rescan · F7 / Shift+F7 next/previous change · Ctrl+F find · Ctrl+C copy selection · Ctrl+B toggle tree",
                );
            });
        });
        if let Some((left, right)) = recent_pick {
            self.left_path = left;
            self.right_path = right;
            self.start_scan(ctx);
        }
    }

    pub(super) fn folder_overview(&mut self, ui: &mut Ui) {
        let Some(tree) = &self.tree else {
            return;
        };
        let idx = self
            .tree_ui
            .selected_rel
            .as_deref()
            .and_then(|rel| tree.idx_by_rel(rel))
            .unwrap_or(tree.root);
        let node = &tree.nodes[idx];
        let counts = node.counts;
        let title = if node.rel.is_empty() {
            "Comparison overview".to_string()
        } else {
            node.rel.clone()
        };
        let is_root = node.rel.is_empty();

        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(20.0).strong());
            if !is_root {
                ui.label(
                    RichText::new("folder")
                        .color(theme::muted())
                        .size(10.0)
                        .strong(),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if counts.differences() > 0
                    && common::intent_button(
                        ui,
                        "First difference",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                {
                    self.deferred.push(UiAction::NextFileDiff { forward: true });
                }
                if counts.differences() > 0
                    && common::intent_button(ui, "Copy all A → B", common::ButtonIntent::Ghost)
                        .clicked()
                {
                    self.deferred.push(UiAction::Sync { from_left: true });
                }
            });
        });
        ui.add_space(6.0);

        path_card(ui, "A", theme::accent(), &tree.left_root, "folder");
        path_card(ui, "B", theme::right(), &tree.right_root, "folder");
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            common::count_chip(ui, "same", counts.same, theme::same());
            common::count_chip(ui, "different", counts.different, theme::diff());
            common::count_chip(ui, "left only", counts.left_only, theme::left());
            common::count_chip(ui, "right only", counts.right_only, theme::right());
            if counts.type_mismatch > 0 {
                common::count_chip(
                    ui,
                    "type mismatch",
                    counts.type_mismatch,
                    theme::type_color(),
                );
            }
            if counts.error > 0 {
                common::count_chip(ui, "errors", counts.error, theme::error());
            }
            common::stat_chip(
                ui,
                "time",
                format!("{} ms", tree.elapsed.as_millis()),
                theme::muted(),
            );
        });

        if let Some(note) = &node.note {
            ui.add_space(4.0);
            ui.label(RichText::new(note).color(theme::diff()).size(11.5));
        }

        ui.add_space(10.0);
        ui.separator();
        ui.horizontal(|ui| {
            common::section_heading(ui, if node.is_dir { "CHANGED FILES" } else { "FILE" });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                common::small_muted(
                    ui,
                    format!(
                        "{} of {} files differ",
                        counts.differences(),
                        counts.total()
                    ),
                );
            });
        });
        ui.add_space(2.0);

        if counts.differences() == 0 {
            ui.add_space(10.0);
            if node.is_dir && node.status.is_difference() {
                ui.label(
                    RichText::new(
                        "The folder structure differs, but there are no changed files in this subtree.",
                    )
                    .color(theme::diff())
                    .size(13.0),
                );
            } else {
                ui.label(
                    RichText::new("No differences in this folder.")
                        .color(theme::same())
                        .size(14.0),
                );
            }
            return;
        }

        let files = tree.differing_files(idx, 500);
        egui::ScrollArea::vertical()
            .id_salt("overview_files")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for file_idx in files {
                    let file = &tree.nodes[file_idx];
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(file.status.glyph())
                                .monospace()
                                .color(theme::status_color(file.status)),
                        );
                        let response = ui.add(
                            Label::new(
                                RichText::new(if file.rel.is_empty() {
                                    file.name.as_str()
                                } else {
                                    file.rel.as_str()
                                })
                                .color(theme::text()),
                            )
                            .truncate()
                            .sense(Sense::click()),
                        );
                        if response.clicked() {
                            self.deferred.push(UiAction::OpenNode(file.rel.clone()));
                        }
                        if let Some(delta) = file.size_delta() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                common::small_muted(ui, format_size_delta(delta));
                            });
                        }
                    });
                }
                ui.add_space(8.0);
            });
    }

    // ---------------------------------------------------------------------
    // Dialogs and actions
    // ---------------------------------------------------------------------
}
