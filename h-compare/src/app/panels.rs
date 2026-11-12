//! Top bar, folder tree panel and status bar.

use super::*;

impl App {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        let mut rescan = false;
        let mut reload_open = false;
        let mut swap = false;
        let mut theme_change = None;
        let mut recent_pick: Option<(String, String)> = None;
        let mut clear_recent = false;
        let scanning = self.scanning();

        egui::TopBottomPanel::top("hcompare_top")
            .frame(
                Frame::NONE
                    .fill(theme::panel())
                    .inner_margin(Margin::symmetric(14, 8))
                    .stroke(Stroke::new(1.0_f32, theme::border())),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    Frame::NONE
                        .fill(theme::accent())
                        .corner_radius(6.0)
                        .inner_margin(Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("H")
                                    .color(theme::on_accent())
                                    .size(19.0)
                                    .strong(),
                            );
                        });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("H-Compare").size(16.0).strong());
                        ui.label(
                            RichText::new(format!(
                                "folder · text · csv diff · v{} build {}",
                                env!("CARGO_PKG_VERSION"),
                                env!("HCOMPARE_BUILD_DATE")
                            ))
                            .color(theme::muted())
                            .size(10.5),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if common::intent_button(ui, "About", common::ButtonIntent::Ghost).clicked()
                        {
                            self.show_about = true;
                        }
                        ui.menu_button(format!("Theme: {}", ui::theme::mode().label()), |ui| {
                            for mode in [ui::theme::ThemeMode::Dark, ui::theme::ThemeMode::Light] {
                                if ui
                                    .selectable_label(self.settings.theme == mode, mode.label())
                                    .clicked()
                                {
                                    theme_change = Some(mode);
                                    ui.close_menu();
                                }
                            }
                        })
                        .response
                        .on_hover_text("Choose the dark or light theme");
                        if common::intent_button(ui, "Options", common::ButtonIntent::Ghost)
                            .clicked()
                        {
                            self.show_settings = true;
                        }
                        ui.menu_button("Recent", |ui| {
                            ui.set_min_width(460.0);
                            if self.settings.recent.is_empty() {
                                common::small_muted(ui, "No recent comparisons yet.");
                            } else {
                                for entry in self.settings.recent.clone() {
                                    let label = format!(
                                        "{}   ↔   {}",
                                        short_path(&entry.left),
                                        short_path(&entry.right)
                                    );
                                    if common::card_button(ui, &label, false)
                                        .on_hover_text(format!("{}\n{}", entry.left, entry.right))
                                        .clicked()
                                    {
                                        recent_pick = Some((entry.left, entry.right));
                                        ui.close_menu();
                                    }
                                }
                                ui.separator();
                                if common::intent_button(
                                    ui,
                                    "Clear recent list",
                                    common::ButtonIntent::Danger,
                                )
                                .clicked()
                                {
                                    clear_recent = true;
                                    ui.close_menu();
                                }
                            }
                        });
                        if let Some(tree) = &self.tree {
                            if !tree.warnings.is_empty()
                                && common::intent_button(
                                    ui,
                                    &format!("Warnings ({})", tree.warnings.len()),
                                    common::ButtonIntent::Ghost,
                                )
                                .clicked()
                            {
                                self.show_warnings = true;
                            }
                        }
                        ui.menu_button("Actions", |ui| {
                            let left_sync = self.sync_candidates(true).len();
                            let right_sync = self.sync_candidates(false).len();
                            let sync_left = ui
                                .add_enabled_ui(left_sync > 0, |ui| {
                                    common::intent_button(
                                        ui,
                                        &format!("Copy all left → right ({left_sync} files)"),
                                        common::ButtonIntent::Secondary,
                                    )
                                })
                                .inner
                                .clicked();
                            if sync_left {
                                self.deferred.push(UiAction::Sync { from_left: true });
                                ui.close_menu();
                            }
                            let sync_right = ui
                                .add_enabled_ui(right_sync > 0, |ui| {
                                    common::intent_button(
                                        ui,
                                        &format!("Copy all right → left ({right_sync} files)"),
                                        common::ButtonIntent::Secondary,
                                    )
                                })
                                .inner
                                .clicked();
                            if sync_right {
                                self.deferred.push(UiAction::Sync { from_left: false });
                                ui.close_menu();
                            }
                            ui.separator();
                            if common::intent_button(
                                ui,
                                "Clear comparison",
                                common::ButtonIntent::Danger,
                            )
                            .clicked()
                            {
                                self.tree = None;
                                self.open = None;
                                self.open_error = None;
                                self.pending_open = None;
                                self.load_token = self.load_token.wrapping_add(1);
                                ui.close_menu();
                            }
                        });
                        if scanning {
                            ui.spinner();
                            common::small_muted(ui, "Scanning…");
                        } else if common::accent_button(ui, "Compare")
                            .on_hover_text("Scan and compare the two folders (F5)")
                            .clicked()
                        {
                            rescan = true;
                        }
                        if common::intent_button(ui, "Rescan", common::ButtonIntent::Ghost)
                            .on_hover_text("F5")
                            .clicked()
                        {
                            rescan = true;
                        }
                        if common::intent_button(ui, "Swap", common::ButtonIntent::Ghost)
                            .on_hover_text("Swap left and right")
                            .clicked()
                        {
                            swap = true;
                        }
                        ui.menu_button("Quick options", |ui| {
                            if ui
                                .checkbox(
                                    &mut self.settings.compare.skip_build_dirs,
                                    "Skip common build folders",
                                )
                                .changed()
                            {
                                rescan = true;
                            }
                            if ui
                                .checkbox(
                                    &mut self.settings.compare.ignore_case,
                                    "Text: ignore case",
                                )
                                .changed()
                            {
                                reload_open = true;
                            }
                            if ui
                                .checkbox(
                                    &mut self.settings.compare.ignore_whitespace,
                                    "Text: ignore whitespace",
                                )
                                .changed()
                            {
                                reload_open = true;
                            }
                            if ui
                                .checkbox(
                                    &mut self.settings.compare.trim_trailing,
                                    "Text: trim trailing",
                                )
                                .changed()
                            {
                                reload_open = true;
                            }
                            if ui
                                .checkbox(
                                    &mut self.settings.csv.has_headers,
                                    "CSV: first row is header",
                                )
                                .changed()
                            {
                                reload_open = true;
                            }
                            if ui
                                .checkbox(&mut self.settings.csv.trim_whitespace, "CSV: trim cells")
                                .changed()
                            {
                                reload_open = true;
                            }
                            ui.separator();
                            if common::intent_button(
                                ui,
                                "All options…",
                                common::ButtonIntent::Secondary,
                            )
                            .clicked()
                            {
                                self.show_settings = true;
                                ui.close_menu();
                            }
                        });
                    });
                });

                ui.add_space(4.0);
                let left = path_field(
                    ui,
                    "A",
                    theme::accent(),
                    &mut self.left_path,
                    "Path to the left folder…",
                );
                if left.browse {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose the left folder")
                        .pick_folder()
                    {
                        self.left_path = path.display().to_string();
                        if !self.right_path.trim().is_empty() {
                            rescan = true;
                        }
                    }
                }
                if left.enter && !self.right_path.trim().is_empty() {
                    rescan = true;
                }

                let right = path_field(
                    ui,
                    "B",
                    theme::right(),
                    &mut self.right_path,
                    "Path to the right folder…",
                );
                if right.browse {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose the right folder")
                        .pick_folder()
                    {
                        self.right_path = path.display().to_string();
                        if !self.left_path.trim().is_empty() {
                            rescan = true;
                        }
                    }
                }
                if right.enter && !self.left_path.trim().is_empty() {
                    rescan = true;
                }
            });

        if let Some(mode) = theme_change {
            self.settings.theme = mode;
            self.pending_theme = Some(mode);
            self.persist();
        }
        if clear_recent {
            self.settings.recent.clear();
            self.persist();
        }
        if let Some((left, right)) = recent_pick {
            self.left_path = left;
            self.right_path = right;
            self.start_scan(ctx);
        }
        if swap {
            std::mem::swap(&mut self.left_path, &mut self.right_path);
            if !self.left_path.trim().is_empty() && !self.right_path.trim().is_empty() {
                rescan = true;
            }
        }
        if reload_open && !rescan {
            self.deferred.push(UiAction::ReloadOpen);
        }
        if rescan {
            self.start_scan(ctx);
        }
    }

    pub(super) fn left_panel(&mut self, ctx: &egui::Context) {
        if self.settings.tree_collapsed {
            egui::SidePanel::left("hcompare_tree_collapsed")
                .exact_width(34.0)
                .resizable(false)
                .frame(
                    Frame::NONE
                        .fill(theme::panel())
                        .inner_margin(Margin::symmetric(4, 8))
                        .stroke(Stroke::new(1.0_f32, theme::border())),
                )
                .show(ctx, |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("»").color(theme::on_accent()).size(16.0),
                            )
                            .fill(theme::accent())
                            .stroke(Stroke::new(1.0_f32, theme::accent()))
                            .corner_radius(8.0)
                            .min_size(egui::vec2(26.0, 26.0)),
                        )
                        .on_hover_text("Show folder tree (Ctrl+B)")
                        .clicked()
                    {
                        self.settings.tree_collapsed = false;
                        self.persist();
                    }
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        for ch in "FILES".chars() {
                            ui.label(
                                RichText::new(ch.to_string())
                                    .color(theme::muted())
                                    .size(9.0),
                            );
                        }
                    });
                });
            return;
        }
        egui::SidePanel::left("hcompare_tree")
            .default_width(372.0)
            .width_range(280.0..=640.0)
            .resizable(true)
            .frame(
                Frame::NONE
                    .fill(theme::panel())
                    .inner_margin(Margin::symmetric(10, 8))
                    .stroke(Stroke::new(1.0_f32, theme::border())),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    common::section_heading(ui, "FOLDERS");
                    if self.scanning() {
                        ui.spinner();
                        common::small_muted(ui, "scanning…");
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if common::intent_button(ui, "«", common::ButtonIntent::Ghost)
                            .on_hover_text("Collapse folder tree (Ctrl+B)")
                            .clicked()
                        {
                            self.settings.tree_collapsed = true;
                            self.persist();
                        }
                        if let Some(tree) = &self.tree {
                            common::small_muted(
                                ui,
                                format!("{} files", tree.counts.total()),
                            );
                        }
                    });
                });
                ui.separator();
                if let Some(tree) = &self.tree {
                    ui::tree::show(
                        ui,
                        tree,
                        &mut self.tree_ui,
                        &mut self.deferred,
                    );
                } else {
                    ui.add_space(16.0);
                    common::small_muted(
                        ui,
                        "Choose two folders above, then press Compare. You can also drop two folders onto the window.",
                    );
                }
            });
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("hcompare_status")
            .frame(
                Frame::NONE
                    .fill(theme::panel())
                    .inner_margin(Margin::symmetric(14, 5))
                    .stroke(Stroke::new(1.0_f32, theme::border())),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.scanning() {
                        ui.spinner();
                        common::small_muted(ui, "Scanning folders…");
                    } else if let Some(tree) = &self.tree {
                        colored_count(ui, "same", tree.counts.same, theme::same());
                        colored_count(ui, "different", tree.counts.different, theme::diff());
                        colored_count(ui, "left only", tree.counts.left_only, theme::left());
                        colored_count(ui, "right only", tree.counts.right_only, theme::right());
                        if tree.counts.error > 0 {
                            colored_count(ui, "errors", tree.counts.error, theme::error());
                        }
                        common::small_muted(
                            ui,
                            format!(
                                "· {} files · {} ms",
                                tree.counts.total(),
                                tree.elapsed.as_millis()
                            ),
                        );
                    } else {
                        common::small_muted(ui, "Ready. Choose two folders to begin.");
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(message) = &self.message {
                            if message.at.elapsed() < Duration::from_secs(7) {
                                ui.label(
                                    RichText::new(&message.text)
                                        .color(if message.error {
                                            theme::error()
                                        } else {
                                            theme::same()
                                        })
                                        .size(11.5),
                                );
                                ctx.request_repaint_after(Duration::from_millis(250));
                            }
                        } else {
                            common::small_muted(
                                ui,
                                "F5 rescan · F7 next change · Ctrl+F find · Ctrl+C copy",
                            );
                        }
                    });
                });
            });
    }
}
