//! Top bar, folder tree panel and status bar.

use super::*;

impl App {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        let mut rescan = false;
        let mut swap = false;
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
                        .corner_radius(9.0)
                        .inner_margin(Margin::symmetric(9, 4))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("H")
                                    .color(theme::on_accent())
                                    .size(19.0)
                                    .strong(),
                            );
                        });
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("H-Compare").size(16.0).strong());
                            Frame::NONE
                                .fill(theme::accent_dim())
                                .corner_radius(999.0)
                                .inner_margin(Margin::symmetric(7, 1))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                            .color(theme::accent())
                                            .size(9.5),
                                    );
                                });
                        });
                        ui.label(
                            RichText::new("folder · text · csv · word")
                                .color(theme::muted())
                                .size(10.5),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if common::icon_button(
                            ui,
                            Icon::Info,
                            "About",
                            common::ButtonIntent::Secondary,
                        )
                        .clicked()
                        {
                            self.show_about = true;
                        }
                        let _ = common::icon_menu_button_intent(
                            ui,
                            Icon::Settings,
                            "Settings",
                            common::ButtonIntent::Secondary,
                            |ui| {
                                ui.set_min_width(280.0);
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(14.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    ui::icons::paint(
                                        ui.painter(),
                                        rect,
                                        Icon::Palette,
                                        theme::accent(),
                                    );
                                    common::section_heading(ui, "APPEARANCE");
                                });
                                if let Some(theme) = egui::ScrollArea::vertical()
                                    .id_salt("theme_picker_scroll")
                                    .max_height(420.0)
                                    .show(ui, |ui| common::theme_picker(ui, self.settings.theme))
                                    .inner
                                {
                                    self.settings.theme = theme;
                                    self.pending_theme = Some(theme);
                                    self.persist();
                                    common::close_menu(ui);
                                }
                                ui.separator();
                                if common::icon_button(
                                    ui,
                                    Icon::Sliders,
                                    "All options…",
                                    common::ButtonIntent::Secondary,
                                )
                                .clicked()
                                {
                                    self.show_settings = true;
                                    common::close_menu(ui);
                                }
                            },
                        );
                        if let Some(tree) = &self.tree {
                            if !tree.warnings.is_empty()
                                && common::icon_button(
                                    ui,
                                    Icon::Warning,
                                    &format!("Warnings ({})", tree.warnings.len()),
                                    common::ButtonIntent::Secondary,
                                )
                                .on_hover_text("Review scan warnings")
                                .clicked()
                            {
                                self.show_warnings = true;
                            }
                        }
                        let _ = common::icon_menu_button_intent(
                            ui,
                            Icon::More,
                            "Actions",
                            common::ButtonIntent::Secondary,
                            |ui| {
                                let left_sync = self.sync_candidates(true).len();
                                let right_sync = self.sync_candidates(false).len();
                                let sync_left = ui
                                    .add_enabled_ui(left_sync > 0, |ui| {
                                        common::icon_button(
                                            ui,
                                            Icon::Copy,
                                            &format!("Copy all A → B ({left_sync})"),
                                            common::ButtonIntent::Secondary,
                                        )
                                    })
                                    .inner
                                    .clicked();
                                if sync_left {
                                    self.deferred.push(UiAction::Sync { from_left: true });
                                    common::close_menu(ui);
                                }
                                let sync_right = ui
                                    .add_enabled_ui(right_sync > 0, |ui| {
                                        common::icon_button(
                                            ui,
                                            Icon::Copy,
                                            &format!("Copy all B → A ({right_sync})"),
                                            common::ButtonIntent::Secondary,
                                        )
                                    })
                                    .inner
                                    .clicked();
                                if sync_right {
                                    self.deferred.push(UiAction::Sync { from_left: false });
                                    common::close_menu(ui);
                                }
                                ui.separator();
                                if common::icon_button(
                                    ui,
                                    Icon::Trash,
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
                                    common::close_menu(ui);
                                }
                            },
                        );
                        let _ = common::icon_menu_button_intent(
                            ui,
                            Icon::Clock,
                            "Recent",
                            common::ButtonIntent::Secondary,
                            |ui| {
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
                                            .on_hover_text(format!(
                                                "{}\n{}",
                                                entry.left, entry.right
                                            ))
                                            .clicked()
                                        {
                                            recent_pick = Some((entry.left, entry.right));
                                            common::close_menu(ui);
                                        }
                                    }
                                    ui.separator();
                                    if common::icon_button(
                                        ui,
                                        Icon::Trash,
                                        "Clear recent list",
                                        common::ButtonIntent::Danger,
                                    )
                                    .clicked()
                                    {
                                        clear_recent = true;
                                        common::close_menu(ui);
                                    }
                                }
                            },
                        );
                        if common::icon_button(
                            ui,
                            Icon::Swap,
                            "Swap",
                            common::ButtonIntent::Secondary,
                        )
                        .on_hover_text("Swap left and right folders")
                        .clicked()
                        {
                            swap = true;
                        }
                        if scanning {
                            ui.spinner();
                            common::small_muted(ui, "Scanning…");
                        } else {
                            let label = if self.tree.is_some() {
                                "Rescan"
                            } else {
                                "Compare"
                            };
                            if common::icon_button(
                                ui,
                                Icon::Refresh,
                                label,
                                common::ButtonIntent::Primary,
                            )
                            .on_hover_text("Compare both folders (F5)")
                            .clicked()
                            {
                                rescan = true;
                            }
                        }
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
        if rescan {
            self.start_scan(ctx);
        }
    }

    pub(super) fn left_panel(&mut self, ctx: &egui::Context) {
        if self.settings.tree_collapsed {
            egui::SidePanel::left("hcompare_tree_collapsed")
                .exact_width(46.0)
                .resizable(false)
                .frame(
                    Frame::NONE
                        .fill(theme::panel())
                        .inner_margin(Margin::symmetric(5, 8))
                        .stroke(Stroke::new(1.0_f32, theme::border())),
                )
                .show(ctx, |ui| {
                    if common::icon_button(
                        ui,
                        Icon::ChevronRight,
                        "",
                        common::ButtonIntent::Primary,
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
                        if common::icon_button(
                            ui,
                            Icon::ChevronLeft,
                            "Hide",
                            common::ButtonIntent::Ghost,
                        )
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
