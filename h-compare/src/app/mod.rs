//! Application state, update loop and shared state types.

mod actions;
mod central;
mod dialogs;
mod panels;
mod widgets;

use widgets::*;

use crate::actions::{ExportKind, UiAction};
use crate::csvdiff::{
    compare_tables_with_ignore, delete_column_from_file, restore_column_backup, CsvIgnore,
};
use crate::folder::compare_folders;
use crate::load::{load_compare, OpenData, OpenKind};
use crate::model::{format_modified, format_size, format_size_delta, CompareTree, EntryStatus};
use crate::report;
use crate::settings::{RecentCompare, Settings};
use crate::textdiff;
use crate::ui;
use crate::ui::common;
use crate::ui::csv_view::CsvViewMode;
use crate::ui::icons::Icon;
use crate::ui::theme;
use eframe::egui::{
    self, Align, Align2, Color32, Frame, Key, Label, Layout, Margin, RichText, Sense, Stroke,
    TextEdit, Ui, Vec2,
};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

pub struct App {
    pub(crate) settings: Settings,
    pub(crate) left_path: String,
    pub(crate) right_path: String,
    pub(crate) tree: Option<CompareTree>,
    pub(crate) tree_ui: ui::tree::TreeState,
    pub(crate) open: Option<OpenState>,
    pub(crate) open_error: Option<(String, String)>,
    pub(crate) pending_open: Option<PendingOpen>,
    pub(crate) load_token: u64,
    pub(crate) deferred: Vec<UiAction>,
    pub(crate) confirm: Option<ConfirmAction>,
    pub(crate) show_settings: bool,
    pub(crate) show_about: bool,
    pub(crate) show_warnings: bool,
    pub(crate) message: Option<Message>,
    pub(crate) pending_theme: Option<ui::theme::ThemeMode>,
    initial_open: Option<String>,
    csv_ignores: HashMap<String, CsvIgnore>,
    csv_column_undo: Option<CsvColumnUndo>,
    /// Backups already restored in this session, so the restore menu does not
    /// offer the same backup twice.
    csv_restored_backups: BTreeSet<PathBuf>,
    scan_rx: Option<Receiver<Result<CompareTree, String>>>,
    scan_started: Option<Instant>,
    scan_error: Option<String>,
}

pub(crate) struct OpenState {
    pub data: OpenData,
    pub text_ui: Option<ui::text_view::TextUiState>,
    pub csv_ui: Option<ui::csv_view::CsvUiState>,
}

/// The most recent column deletion, kept so `Ctrl+Z` (or the toolbar Undo
/// button) can put the column back from its automatic backups.
#[derive(Debug, Clone)]
pub(crate) struct CsvColumnUndo {
    pub column: usize,
    pub entries: Vec<CsvColumnUndoEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct CsvColumnUndoEntry {
    pub path: PathBuf,
    pub backup: PathBuf,
}

impl OpenState {
    fn new(data: OpenData) -> Self {
        let text_ui = data.text.as_ref().map(ui::text_view::TextUiState::new);
        let csv_ui = data.csv.as_ref().map(ui::csv_view::CsvUiState::new);
        Self {
            data,
            text_ui,
            csv_ui,
        }
    }
}

pub(crate) struct PendingOpen {
    token: u64,
    rel: String,
    rx: Receiver<Result<OpenData, String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct Message {
    text: String,
    at: Instant,
    error: bool,
}

#[derive(Debug)]
pub(crate) struct ConfirmAction {
    message: String,
    kind: ConfirmKind,
}

#[derive(Debug)]
enum ConfirmKind {
    Copy {
        rel: String,
        from_left: bool,
    },
    Delete {
        rel: String,
        left: bool,
    },
    DeleteCsvColumn {
        path: PathBuf,
        column: usize,
        delimiter: u8,
    },
    DeleteCsvColumnBoth {
        targets: Vec<(PathBuf, u8)>,
        column: usize,
    },
    Sync {
        from_left: bool,
        files: Vec<String>,
    },
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        left: Option<String>,
        right: Option<String>,
        initial_open: Option<String>,
        theme: Option<ui::theme::ThemeMode>,
    ) -> Self {
        let mut settings = Settings::load().unwrap_or_default();
        if let Some(theme) = theme {
            settings.theme = theme;
        }
        ui::theme::set_mode(settings.theme);
        ui::theme::install(&cc.egui_ctx);
        if let Some(left) = left {
            settings.left = left;
        }
        if let Some(right) = right {
            settings.right = right;
        }
        let mut app = Self {
            left_path: settings.left.clone(),
            right_path: settings.right.clone(),
            settings,
            tree: None,
            tree_ui: ui::tree::TreeState::default(),
            open: None,
            open_error: None,
            pending_open: None,
            load_token: 0,
            deferred: Vec::new(),
            confirm: None,
            show_settings: false,
            show_about: false,
            show_warnings: false,
            message: None,
            pending_theme: None,
            initial_open,
            csv_ignores: HashMap::new(),
            csv_column_undo: None,
            csv_restored_backups: BTreeSet::new(),
            scan_rx: None,
            scan_started: None,
            scan_error: None,
        };
        if !app.left_path.trim().is_empty() && !app.right_path.trim().is_empty() {
            app.start_scan(&cc.egui_ctx);
        }
        app
    }

    fn scanning(&self) -> bool {
        self.scan_rx.is_some()
    }

    fn search_is_open(&self) -> bool {
        self.open.as_ref().is_some_and(|open| {
            open.text_ui.as_ref().is_some_and(|ui| ui.search.open)
                || open.csv_ui.as_ref().is_some_and(|ui| ui.search.open)
        })
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.scan_rx.is_some() {
            return;
        }
        let left = self.left_path.trim().to_string();
        let right = self.right_path.trim().to_string();
        if left.is_empty() || right.is_empty() {
            self.set_message("Choose both a left and a right folder first.", true);
            return;
        }
        self.settings.left = left.clone();
        self.settings.right = right.clone();
        let _ = self.settings.save();
        let options = self.settings.compare.clone();
        let left_path = PathBuf::from(left);
        let right_path = PathBuf::from(right);
        let same_pair = self
            .tree
            .as_ref()
            .is_some_and(|tree| tree.left_root == left_path && tree.right_root == right_path);
        if !same_pair {
            self.csv_ignores.clear();
            self.csv_column_undo = None;
            self.csv_restored_backups.clear();
        }
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let result = compare_folders(&left_path, &right_path, &options);
            let _ = tx.send(result);
            repaint.request_repaint();
        });
        self.scan_rx = Some(rx);
        self.scan_started = Some(Instant::now());
        self.scan_error = None;
        self.open = None;
        self.open_error = None;
        self.pending_open = None;
        self.load_token = self.load_token.wrapping_add(1);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            "H-Compare — scanning folders…".to_string(),
        ));
    }

    fn apply_tree(&mut self, tree: CompareTree, ctx: &egui::Context) {
        let previous = self.tree_ui.selected_rel.clone();
        let initial = self.initial_open.take();
        let filter = self.tree_ui.filter;
        let query = self.tree_ui.query.clone();
        let chosen_idx = initial
            .as_deref()
            .and_then(|rel| tree.idx_by_rel(rel))
            .or_else(|| previous.as_deref().and_then(|rel| tree.idx_by_rel(rel)))
            .or_else(|| tree.first_difference())
            .unwrap_or(tree.root);
        let chosen_rel = tree.node(chosen_idx).rel.clone();
        let is_file = !tree.node(chosen_idx).is_dir;
        let title = format!(
            "H-Compare — {} vs {}",
            short_root(&tree.left_root),
            short_root(&tree.right_root)
        );
        self.tree = Some(tree);
        let mut state = ui::tree::TreeState {
            filter,
            query,
            ..Default::default()
        };
        if let Some(tree) = &self.tree {
            state.reset(tree, Some(chosen_rel.clone()));
        }
        self.tree_ui = state;
        self.scan_error = None;
        self.open = None;
        self.open_error = None;
        self.pending_open = None;
        self.load_token = self.load_token.wrapping_add(1);
        self.remember_recent();
        self.persist();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        if is_file && !chosen_rel.is_empty() {
            self.start_open(&chosen_rel, ctx);
        }
    }

    fn start_open(&mut self, rel: &str, ctx: &egui::Context) {
        let Some(tree) = &self.tree else {
            return;
        };
        let Some(node) = tree.by_rel(rel) else {
            return;
        };
        if node.is_dir || rel.is_empty() {
            self.open = None;
            self.open_error = None;
            self.pending_open = None;
            self.load_token = self.load_token.wrapping_add(1);
            return;
        }
        let left = node.left_path().map(Path::to_path_buf);
        let right = node.right_path().map(Path::to_path_buf);
        let compare_options = self.settings.compare.clone();
        let csv_options = self.settings.csv.clone();
        let rel_owned = rel.to_string();
        self.load_token = self.load_token.wrapping_add(1);
        let token = self.load_token;
        self.tree_ui.selected_rel = Some(rel_owned.clone());
        self.open = None;
        self.open_error = None;
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let result = load_compare(&rel_owned, left, right, &compare_options, &csv_options);
            let _ = tx.send(Ok::<OpenData, String>(result));
            repaint.request_repaint();
        });
        self.pending_open = Some(PendingOpen {
            token,
            rel: rel.to_string(),
            rx,
        });
    }

    fn reload_open(&mut self, ctx: &egui::Context) {
        if let Some(rel) = self.tree_ui.selected_rel.clone() {
            if self
                .tree
                .as_ref()
                .and_then(|tree| tree.by_rel(&rel))
                .map(|node| !node.is_dir)
                .unwrap_or(false)
            {
                self.start_open(&rel, ctx);
            }
        }
    }

    fn poll_jobs(&mut self, ctx: &egui::Context) {
        let scan_outcome = self.scan_rx.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err("The folder scanner stopped unexpectedly.".to_string()))
            }
        });
        if let Some(outcome) = scan_outcome {
            self.scan_rx = None;
            self.scan_started = None;
            match outcome {
                Ok(tree) => self.apply_tree(tree, ctx),
                Err(error) => {
                    self.scan_error = Some(error.clone());
                    self.set_message(error, true);
                }
            }
            ctx.request_repaint();
        }

        let outcome = self
            .pending_open
            .as_ref()
            .and_then(|pending| match pending.rx.try_recv() {
                Ok(result) => Some((pending.token, pending.rel.clone(), result)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some((
                    pending.token,
                    pending.rel.clone(),
                    Err("The file loader stopped unexpectedly.".to_string()),
                )),
            });
        if let Some((token, rel, result)) = outcome {
            self.pending_open = None;
            if token == self.load_token {
                match result {
                    Ok(data) => {
                        let rel_key = data.rel.clone();
                        let mut state = OpenState::new(data);
                        if let Some(text_ui) = &mut state.text_ui {
                            text_ui.options_collapsed = self.settings.text_options_collapsed;
                        }
                        if let Some(csv_ui) = &mut state.csv_ui {
                            csv_ui.mode = if self.settings.csv_side_by_side {
                                CsvViewMode::SideBySide
                            } else {
                                CsvViewMode::Merged
                            };
                            csv_ui.differences_only = self.settings.csv_differences_only;
                            csv_ui.options_collapsed = self.settings.csv_options_collapsed;
                            csv_ui.restored_backups = self.csv_restored_backups.clone();
                        }
                        self.open = Some(state);
                        self.open_error = None;
                        if let Some(ignore) = self.csv_ignores.get(&rel_key).cloned() {
                            self.recompute_csv_ignore(&ignore);
                        }
                    }
                    Err(error) => {
                        self.open = None;
                        self.open_error = Some((rel, error));
                    }
                }
                ctx.request_repaint();
            }
        }
    }

    fn set_message(&mut self, text: impl Into<String>, error: bool) {
        self.message = Some(Message {
            text: text.into(),
            at: Instant::now(),
            error,
        });
    }

    fn persist(&mut self) {
        self.settings.left = self.left_path.clone();
        self.settings.right = self.right_path.clone();
        let _ = self.settings.save();
    }

    fn remember_recent(&mut self) {
        let left = self.left_path.trim().to_string();
        let right = self.right_path.trim().to_string();
        if left.is_empty() || right.is_empty() {
            return;
        }
        self.settings
            .recent
            .retain(|entry| entry.left != left || entry.right != right);
        self.settings
            .recent
            .insert(0, RecentCompare { left, right });
        self.settings.recent.truncate(12);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.key_pressed(Key::F5)) {
            self.start_scan(ctx);
        }
        if ctx.input(|input| input.key_pressed(Key::Enter))
            && !ctx.wants_keyboard_input()
            && !self.scanning()
            && self.tree.is_none()
            && self.confirm.is_none()
            && !self.show_settings
            && !self.show_about
            && !self.show_warnings
            && !self.search_is_open()
            && !self.left_path.trim().is_empty()
            && !self.right_path.trim().is_empty()
        {
            self.start_scan(ctx);
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(Key::B)) {
            self.settings.tree_collapsed = !self.settings.tree_collapsed;
            self.persist();
            ctx.request_repaint();
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(Key::F)) {
            if let Some(open) = &mut self.open {
                if let Some(text_ui) = &mut open.text_ui {
                    text_ui.search.open_and_focus();
                }
                if let Some(csv_ui) = &mut open.csv_ui {
                    csv_ui.search.open_and_focus();
                }
            }
        }
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            let mut closed_search = false;
            if let Some(open) = &mut self.open {
                if let Some(text_ui) = &mut open.text_ui {
                    if text_ui.search.open {
                        text_ui.search.open = false;
                        closed_search = true;
                    }
                }
                if let Some(csv_ui) = &mut open.csv_ui {
                    if csv_ui.search.open {
                        csv_ui.search.open = false;
                        closed_search = true;
                    }
                }
            }
            if !closed_search {
                self.confirm = None;
                self.show_settings = false;
                self.show_about = false;
                self.show_warnings = false;
            }
        }
        if ctx.input(|input| input.key_pressed(Key::F7)) {
            let forward = !ctx.input(|input| input.modifiers.shift);
            self.navigate(forward, ctx);
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(Key::C))
            && !ctx.wants_keyboard_input()
        {
            self.copy_selected(ctx);
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(Key::Z))
            && !ctx.wants_keyboard_input()
        {
            self.deferred.push(UiAction::UndoCsvColumnDelete);
        }
    }

    fn navigate(&mut self, forward: bool, ctx: &egui::Context) {
        let mut handled = false;
        if let Some(open) = &mut self.open {
            if let (Some(view), Some(text_ui)) = (open.data.text.as_ref(), open.text_ui.as_mut()) {
                handled = text_ui.goto_change(view, forward);
            } else if let (Some(compare), Some(csv_ui)) =
                (open.data.csv.as_ref(), open.csv_ui.as_mut())
            {
                handled = csv_ui.goto_change(compare, forward);
            }
        }
        if handled {
            ctx.request_repaint();
            return;
        }
        self.next_file_diff(forward, ctx);
    }

    fn next_file_diff(&mut self, forward: bool, ctx: &egui::Context) {
        let target = {
            let Some(tree) = &self.tree else {
                return;
            };
            let current = self
                .tree_ui
                .selected_rel
                .as_deref()
                .and_then(|rel| tree.idx_by_rel(rel));
            let target = if forward {
                tree.next_difference(current)
                    .or_else(|| tree.first_difference())
            } else {
                tree.previous_difference(current)
                    .or_else(|| tree.previous_difference(None))
            };
            target.map(|target| {
                (
                    tree.node(target).rel.clone(),
                    tree.ancestors(target)
                        .into_iter()
                        .map(|idx| tree.node(idx).rel.clone())
                        .collect::<Vec<_>>(),
                )
            })
        };
        let Some((rel, ancestors)) = target else {
            self.set_message("No differences found in this comparison.", false);
            return;
        };
        for ancestor in ancestors {
            self.tree_ui.expanded.insert(ancestor);
        }
        self.tree_ui.expanded.insert(rel.clone());
        self.start_open(&rel, ctx);
    }

    fn copy_selected(&mut self, ctx: &egui::Context) {
        let mut copied = None;
        if let Some(open) = &self.open {
            if let (Some(view), Some(text_ui)) = (open.data.text.as_ref(), open.text_ui.as_ref()) {
                if let Some(row_idx) = text_ui.selected_row {
                    if let Some(row) = view.rows.get(row_idx) {
                        let text = match (row.left, row.right) {
                            (Some(left), Some(right)) => {
                                format!("{}\t{}", view.left_line(left), view.right_line(right))
                            }
                            (Some(left), None) => view.left_line(left).to_string(),
                            (None, Some(right)) => view.right_line(right).to_string(),
                            (None, None) => String::new(),
                        };
                        if !text.is_empty() {
                            copied = Some(text);
                        }
                    }
                }
            } else if let (Some(compare), Some(csv_ui)) =
                (open.data.csv.as_ref(), open.csv_ui.as_ref())
            {
                if let Some(row_idx) = csv_ui.selected_row {
                    if let Some(row) = compare.rows.get(row_idx) {
                        let text = if let Some((_, col)) = csv_ui.selected_cell {
                            row.right
                                .map(|idx| compare.right.cell(idx, col).to_string())
                                .or_else(|| {
                                    row.left.map(|idx| compare.left.cell(idx, col).to_string())
                                })
                        } else {
                            row.right
                                .map(|idx| compare.right.rows[idx].join("\t"))
                                .or_else(|| row.left.map(|idx| compare.left.rows[idx].join("\t")))
                        };
                        copied = text.filter(|text| !text.is_empty());
                    }
                }
            }
        }
        if let Some(text) = copied {
            ctx.copy_text(text);
            self.set_message("Copied to clipboard.", false);
        } else {
            self.set_message("Select a line or cell to copy first.", false);
        }
    }

    fn handle_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        let dirs: Vec<PathBuf> = dropped.into_iter().filter(|path| path.is_dir()).collect();
        if dirs.is_empty() {
            self.set_message("Drop folders, not individual files.", true);
            return;
        }
        if dirs.len() >= 2 {
            self.left_path = dirs[0].display().to_string();
            self.right_path = dirs[1].display().to_string();
        } else if self.left_path.trim().is_empty() {
            self.left_path = dirs[0].display().to_string();
        } else {
            self.right_path = dirs[0].display().to_string();
        }
        if !self.left_path.trim().is_empty() && !self.right_path.trim().is_empty() {
            self.start_scan(ctx);
        }
    }

    // ---------------------------------------------------------------------
    // UI
    // ---------------------------------------------------------------------
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_jobs(ctx);
        self.handle_dropped(ctx);
        self.handle_shortcuts(ctx);
        self.top_bar(ctx);
        self.left_panel(ctx);
        self.status_bar(ctx);
        self.central_panel(ctx);
        self.sync_csv_view_state();
        self.sync_text_view_state();
        self.dialogs(ctx);
        self.process_deferred(ctx);
        if let Some(mode) = self.pending_theme.take() {
            ui::theme::set_mode(mode);
            ui::theme::apply_visuals(ctx);
            ctx.request_repaint();
        }
        if self.scanning() || self.pending_open.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}

// -------------------------------------------------------------------------
// Small helpers and free-standing panels
// -------------------------------------------------------------------------
