//! Folder tree panel with filters, expansion and context actions.

use crate::actions::UiAction;
use crate::model::{
    format_modified, format_size, format_size_delta, CompareTree, EntryStatus, StatusFilter,
};
use crate::ui::common::{self, selectable_chip, small_muted};
use crate::ui::theme;
use egui::{Align2, FontId, RichText, Sense, Ui, Vec2};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct TreeState {
    pub expanded: HashSet<String>,
    pub filter: StatusFilter,
    pub query: String,
    pub selected_rel: Option<String>,
}

impl TreeState {
    pub fn reset(&mut self, tree: &CompareTree, selected: Option<String>) {
        self.expanded.clear();
        self.expanded.insert(String::new());
        self.selected_rel = selected.clone();
        if let Some(rel) = selected {
            if let Some(idx) = tree.idx_by_rel(&rel) {
                for ancestor in tree.ancestors(idx) {
                    self.expanded.insert(tree.node(ancestor).rel.clone());
                }
            }
        }
    }

    pub fn expand_all(&mut self, tree: &CompareTree) {
        for node in &tree.nodes {
            if node.is_dir {
                self.expanded.insert(node.rel.clone());
            }
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.expanded.insert(String::new());
    }
}

const ROW_HEIGHT: f32 = 23.0;
const INDENT: f32 = 14.0;

pub fn show(ui: &mut Ui, tree: &CompareTree, state: &mut TreeState, actions: &mut Vec<UiAction>) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        small_muted(ui, "FILTER BY NAME");
        ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text("*.csv, reports, …")
                .desired_width(f32::INFINITY),
        );
        if !state.query.is_empty() && ui.small_button("×").clicked() {
            state.query.clear();
        }
    });
    ui.add_space(2.0);

    let counts = tree.counts;
    ui.horizontal_wrapped(|ui| {
        if selectable_chip(
            ui,
            state.filter == StatusFilter::All,
            "All",
            counts.total(),
            theme::muted(),
        )
        .clicked()
        {
            state.filter = StatusFilter::All;
        }
        if selectable_chip(
            ui,
            state.filter == StatusFilter::Differences,
            "Diffs",
            counts.differences(),
            theme::diff(),
        )
        .clicked()
        {
            state.filter = StatusFilter::Differences;
        }
        if selectable_chip(
            ui,
            state.filter == StatusFilter::Same,
            "Same",
            counts.same,
            theme::same(),
        )
        .clicked()
        {
            state.filter = StatusFilter::Same;
        }
        if selectable_chip(
            ui,
            state.filter == StatusFilter::LeftOnly,
            "Left",
            counts.left_only,
            theme::left(),
        )
        .clicked()
        {
            state.filter = StatusFilter::LeftOnly;
        }
        if selectable_chip(
            ui,
            state.filter == StatusFilter::RightOnly,
            "Right",
            counts.right_only,
            theme::right(),
        )
        .clicked()
        {
            state.filter = StatusFilter::RightOnly;
        }
    });

    ui.horizontal(|ui| {
        if common::intent_button(ui, "⊞ Expand all", common::ButtonIntent::Secondary).clicked() {
            state.expand_all(tree);
        }
        if common::intent_button(ui, "⊟ Collapse all", common::ButtonIntent::Secondary).clicked()
        {
            state.collapse_all();
        }
        if !tree.warnings.is_empty() {
            ui.label(
                RichText::new(format!("{} warnings", tree.warnings.len()))
                    .color(theme::diff())
                    .size(11.0),
            );
        }
    });

    let filtering = state.filter != StatusFilter::All || !state.query.is_empty();
    let query = state.query.to_lowercase();
    let mut memo = HashMap::new();
    let any_visible = if filtering {
        mark_visible(tree, tree.root, state.filter, &query, false, &mut memo)
    } else {
        true
    };

    ui.separator();
    if !any_visible && !filtering {
        small_muted(ui, "No entries.");
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("folder_tree")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut ctx = TreeDrawContext {
                tree,
                state,
                actions,
                memo: &memo,
                filtering,
            };
            if filtering && !any_visible {
                ui.add_space(12.0);
                small_muted(ui, "Nothing matches the current filter.");
            } else {
                draw_node(ui, &mut ctx, tree.root, true);
            }
            ui.add_space(12.0);
        });
}

fn mark_visible(
    tree: &CompareTree,
    idx: usize,
    filter: StatusFilter,
    query: &str,
    forced: bool,
    memo: &mut HashMap<usize, bool>,
) -> bool {
    let node = &tree.nodes[idx];
    let name_match = query.is_empty()
        || node.name.to_lowercase().contains(query)
        || node.rel.to_lowercase().contains(query);
    let force = forced || (!query.is_empty() && name_match);
    let mut visible = force;
    if node.is_dir {
        for child in &node.children {
            visible |= mark_visible(tree, *child, filter, query, force, memo);
        }
        if node.children.is_empty() && filter.matches(node.status) && query.is_empty() {
            visible = true;
        }
    } else {
        visible |= filter.matches(node.status) && name_match;
    }
    memo.insert(idx, visible);
    visible
}

struct TreeDrawContext<'a> {
    tree: &'a CompareTree,
    state: &'a mut TreeState,
    actions: &'a mut Vec<UiAction>,
    memo: &'a HashMap<usize, bool>,
    filtering: bool,
}

fn draw_node(ui: &mut Ui, ctx: &mut TreeDrawContext<'_>, idx: usize, is_root: bool) {
    if ctx.filtering && ctx.memo.get(&idx).copied() != Some(true) {
        return;
    }
    let node = &ctx.tree.nodes[idx];
    let expanded =
        is_root || ctx.filtering || ctx.state.expanded.contains(&node.rel) || node.rel.is_empty();
    draw_row(ui, ctx, idx, is_root, expanded);

    if node.is_dir && expanded {
        for child in &node.children {
            draw_node(ui, ctx, *child, false);
        }
    }
}

fn draw_row(ui: &mut Ui, ctx: &mut TreeDrawContext<'_>, idx: usize, is_root: bool, expanded: bool) {
    let tree = ctx.tree;
    let state = &mut *ctx.state;
    let actions = &mut *ctx.actions;
    let node = &tree.nodes[idx];
    let selected = if is_root {
        state.selected_rel.as_deref().unwrap_or("").is_empty()
    } else {
        state.selected_rel.as_deref() == Some(node.rel.as_str())
    };

    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click());
    let hover = if node.is_dir {
        node.rel.clone()
    } else {
        let entry = node.left.as_ref().or(node.right.as_ref());
        match entry {
            Some(entry) => format!(
                "{}\n{} · modified {}",
                node.rel,
                format_size(entry.size),
                format_modified(entry.modified)
            ),
            None => node.rel.clone(),
        }
    };
    let response = response.on_hover_text(if node.rel.is_empty() {
        "Comparison root".to_string()
    } else {
        hover
    });
    if response.clicked() {
        state.selected_rel = Some(node.rel.clone());
        if node.is_dir && !is_root {
            if state.expanded.contains(&node.rel) {
                state.expanded.remove(&node.rel);
            } else {
                state.expanded.insert(node.rel.clone());
            }
        }
        actions.push(UiAction::OpenNode(node.rel.clone()));
    }
    response.context_menu(|ui| {
        if common::intent_button(ui, "Open", common::ButtonIntent::Secondary).clicked() {
            actions.push(UiAction::OpenNode(node.rel.clone()));
            ui.close_menu();
        }
        if !node.is_dir {
            if node.left.is_some()
                && common::intent_button(ui, "Copy left → right", common::ButtonIntent::Secondary)
                    .clicked()
            {
                actions.push(UiAction::Copy {
                    rel: node.rel.clone(),
                    from_left: true,
                });
                ui.close_menu();
            }
            if node.right.is_some()
                && common::intent_button(ui, "Copy right → left", common::ButtonIntent::Secondary)
                    .clicked()
            {
                actions.push(UiAction::Copy {
                    rel: node.rel.clone(),
                    from_left: false,
                });
                ui.close_menu();
            }
            if common::intent_button(ui, "Reveal left in Explorer", common::ButtonIntent::Ghost)
                .clicked()
            {
                actions.push(UiAction::Reveal {
                    rel: node.rel.clone(),
                    left: true,
                });
                ui.close_menu();
            }
            if common::intent_button(ui, "Reveal right in Explorer", common::ButtonIntent::Ghost)
                .clicked()
            {
                actions.push(UiAction::Reveal {
                    rel: node.rel.clone(),
                    left: false,
                });
                ui.close_menu();
            }
            if node.left.is_some()
                && common::intent_button(ui, "Delete left file…", common::ButtonIntent::Danger)
                    .clicked()
            {
                actions.push(UiAction::Delete {
                    rel: node.rel.clone(),
                    left: true,
                });
                ui.close_menu();
            }
            if node.right.is_some()
                && common::intent_button(ui, "Delete right file…", common::ButtonIntent::Danger)
                    .clicked()
            {
                actions.push(UiAction::Delete {
                    rel: node.rel.clone(),
                    left: false,
                });
                ui.close_menu();
            }
        }
        if !node.rel.is_empty()
            && common::intent_button(ui, "Copy relative path", common::ButtonIntent::Ghost)
                .clicked()
        {
            ui.ctx().copy_text(node.rel.clone());
            ui.close_menu();
        }
        if let Some(path) = node.left_path().or_else(|| node.right_path()) {
            if common::intent_button(ui, "Copy full path", common::ButtonIntent::Ghost).clicked() {
                ui.ctx().copy_text(path.display().to_string());
                ui.close_menu();
            }
        }
    });

    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if selected {
        painter.rect_filled(rect, 6.0, theme::sel_bg());
    } else if response.hovered() {
        painter.rect_filled(rect, 6.0, theme::hover_bg());
    }

    let mut x = rect.left() + 6.0 + node.depth as f32 * INDENT;
    let arrow = if node.is_dir {
        if expanded {
            "▾"
        } else {
            "▸"
        }
    } else {
        " "
    };
    if node.is_dir {
        painter.text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            arrow,
            FontId::proportional(12.0),
            theme::muted(),
        );
    }
    x += 14.0;

    if node.is_dir {
        painter.circle_filled(
            egui::pos2(x + 3.0, rect.center().y),
            4.0,
            theme::status_color(node.status),
        );
        x += 12.0;
    } else {
        painter.text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            node.status.glyph(),
            FontId::monospace(13.0),
            theme::status_color(node.status),
        );
        x += 14.0;
    }

    let name = if is_root {
        "All files".to_string()
    } else {
        node.name.clone()
    };
    let badge = badge_text(node.status, node, tree);
    let badge_width = if badge.is_empty() {
        0.0
    } else {
        (badge.chars().count() as f32 * 6.6 + 12.0).min(90.0)
    };
    let name_max = (rect.right() - 8.0 - badge_width - x).max(20.0);
    let name = ellipsize(&name, name_max, 6.7);
    painter.text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(13.0),
        if selected {
            theme::on_selection()
        } else {
            theme::text()
        },
    );
    if !badge.is_empty() {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            badge,
            FontId::monospace(10.5),
            theme::status_color(node.status),
        );
    }
}

fn badge_text(status: EntryStatus, node: &crate::model::Node, tree: &CompareTree) -> String {
    if node.is_dir {
        let differences = node.counts.differences();
        if differences > 0 && node.rel != tree.root_node().rel {
            return format!("{differences}");
        }
        String::new()
    } else {
        match status {
            EntryStatus::Different => node.size_delta().map(format_size_delta).unwrap_or_default(),
            EntryStatus::LeftOnly => "A".to_string(),
            EntryStatus::RightOnly => "B".to_string(),
            EntryStatus::TypeMismatch => "type".to_string(),
            EntryStatus::Error => "err".to_string(),
            EntryStatus::Same => String::new(),
        }
    }
}

fn ellipsize(text: &str, max_width: f32, char_width: f32) -> String {
    let max_chars = (max_width / char_width).floor().max(4.0) as usize;
    let count = text.chars().count();
    if count <= max_chars {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
