//! Shared egui widgets, search state and reusable UI helpers.

use crate::textdiff::char_range_to_bytes;
use crate::ui::theme;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Frame, Key, Response, RichText, Stroke, TextEdit, Ui};
use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub struct SearchUiState {
    pub open: bool,
    pub query: String,
    pub case_sensitive: bool,
    /// Row indices containing at least one hit.
    pub matches: Vec<usize>,
    pub index: usize,
    /// Total number of individual hits (may be larger than matches).
    pub count: usize,
    pub focus: bool,
}

impl SearchUiState {
    pub fn current(&self) -> Option<usize> {
        self.matches.get(self.index).copied()
    }

    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.index = (self.index + 1) % self.matches.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.matches.is_empty() {
            self.index = (self.index + self.matches.len() - 1) % self.matches.len();
        }
    }

    pub fn set_matches(&mut self, matches: Vec<usize>, count: usize) -> Option<usize> {
        self.matches = matches;
        self.count = count;
        if self.matches.is_empty() {
            self.index = 0;
            None
        } else {
            self.index = self.index.min(self.matches.len() - 1);
            self.current()
        }
    }

    pub fn open_and_focus(&mut self) {
        self.open = true;
        self.focus = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchUpdate {
    None,
    Changed,
    Navigate,
}

pub fn search_bar(ui: &mut Ui, state: &mut SearchUiState) -> SearchUpdate {
    let mut update = SearchUpdate::None;
    ui.horizontal(|ui| {
        let response = ui.add(
            TextEdit::singleline(&mut state.query)
                .hint_text("Find in this comparison…")
                .desired_width(230.0)
                .id_salt("hcompare_search"),
        );
        if state.focus {
            response.request_focus();
            state.focus = false;
        }
        if response.changed() {
            state.index = 0;
            update = SearchUpdate::Changed;
        }
        if response.has_focus() {
            let (enter, shift) =
                ui.input(|input| (input.key_pressed(Key::Enter), input.modifiers.shift));
            if enter {
                if shift {
                    state.previous();
                } else {
                    state.next();
                }
                update = SearchUpdate::Navigate;
            }
        }
        if ui
            .selectable_label(state.case_sensitive, "Aa")
            .on_hover_text("Match case")
            .clicked()
        {
            state.case_sensitive = !state.case_sensitive;
            update = SearchUpdate::Changed;
        }
        if ui
            .button("◀")
            .on_hover_text("Previous match (Shift+Enter)")
            .clicked()
        {
            state.previous();
            update = SearchUpdate::Navigate;
        }
        if ui.button("▶").on_hover_text("Next match (Enter)").clicked() {
            state.next();
            update = SearchUpdate::Navigate;
        }
        if !state.query.is_empty() {
            if state.matches.is_empty() {
                ui.label(RichText::new("no matches").color(theme::muted()).small());
            } else {
                ui.label(
                    RichText::new(format!(
                        "{} / {} rows · {} hits",
                        state.index + 1,
                        state.matches.len(),
                        state.count
                    ))
                    .color(theme::muted())
                    .small(),
                );
            }
        }
        if ui.button("×").on_hover_text("Close search").clicked() {
            state.open = false;
            state.query.clear();
            state.matches.clear();
            state.count = 0;
            state.index = 0;
            update = SearchUpdate::Changed;
        }
    });
    update
}

/// Build a monospace layout job with background highlights for intra-line diffs
/// and search hits. Search highlighting wins where ranges overlap.
pub fn highlighted_job(
    text: &str,
    font: FontId,
    color: Color32,
    diff_spans: &[Range<usize>],
    diff_bg: Color32,
    search_spans: &[Range<usize>],
    search_bg: Color32,
) -> LayoutJob {
    let mut cuts = vec![0usize, text.len()];
    let diff_bytes: Vec<Range<usize>> = diff_spans
        .iter()
        .filter_map(|span| char_range_to_bytes(text, span.clone()))
        .collect();
    for span in diff_bytes.iter().chain(search_spans.iter()) {
        let start = span.start.min(text.len());
        let end = span.end.min(text.len());
        if start < end && text.is_char_boundary(start) && text.is_char_boundary(end) {
            cuts.push(start);
            cuts.push(end);
        }
    }
    cuts.sort_unstable();
    cuts.dedup();

    let base = TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    if cuts.len() < 2 {
        job.append(text, 0.0, base);
        return job;
    }
    for pair in cuts.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start >= end {
            continue;
        }
        let bg = if search_spans
            .iter()
            .any(|span| span.start < end && span.end > start)
        {
            search_bg
        } else if diff_bytes
            .iter()
            .any(|span| span.start < end && span.end > start)
        {
            diff_bg
        } else {
            Color32::TRANSPARENT
        };
        let mut format = base.clone();
        if bg != Color32::TRANSPARENT {
            format.background = bg;
        }
        job.append(&text[start..end], 0.0, format);
    }
    job
}

pub fn count_chip(ui: &mut Ui, label: impl Into<String>, count: usize, color: Color32) -> Response {
    Frame::NONE
        .inner_margin(egui::Margin::symmetric(9, 3))
        .fill(color.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.45)))
        .corner_radius(10.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{}  {}", label.into(), count))
                    .color(color)
                    .size(11.5),
            );
        })
        .response
}

pub fn stat_chip(ui: &mut Ui, label: impl Into<String>, value: impl Into<String>, color: Color32) {
    Frame::NONE
        .inner_margin(egui::Margin::symmetric(9, 3))
        .fill(theme::panel_2())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .corner_radius(10.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(label.into()).color(theme::muted()).size(11.5));
                ui.label(RichText::new(value.into()).color(color).strong().size(11.5));
            });
        });
}

pub fn accent_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(theme::on_accent()))
            .fill(theme::accent())
            .stroke(Stroke::new(1.0_f32, theme::accent()))
            .corner_radius(8.0)
            .min_size(egui::vec2(88.0, 30.0)),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonIntent {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// Compact button built on egui's native `Button`, so hover, press and click
/// handling are reliable. The intent decides the weight of the button:
/// primary actions are filled, secondary actions are softly outlined, ghost
/// actions stay quiet, and destructive actions use a muted red treatment.
pub fn intent_button(ui: &mut Ui, text: &str, intent: ButtonIntent) -> Response {
    let enabled = ui.is_enabled();
    let (fill, stroke, text_color, hover_fill, hover_text) = match intent {
        ButtonIntent::Primary => (
            theme::accent(),
            theme::accent(),
            theme::on_accent(),
            theme::accent().gamma_multiply(1.15),
            theme::on_accent(),
        ),
        ButtonIntent::Secondary => (
            theme::widget(),
            theme::accent().gamma_multiply(0.65),
            theme::text(),
            theme::widget_hover(),
            theme::text(),
        ),
        ButtonIntent::Ghost => (
            theme::panel_2(),
            theme::border(),
            theme::muted(),
            theme::widget_hover(),
            theme::text(),
        ),
        ButtonIntent::Danger => (
            theme::del_bg(),
            theme::error(),
            theme::error(),
            theme::error(),
            theme::on_error(),
        ),
    };
    ui.scope(|ui| {
        let widgets = &mut ui.style_mut().visuals.widgets;
        widgets.inactive.weak_bg_fill = fill;
        widgets.inactive.bg_fill = fill;
        widgets.inactive.bg_stroke = Stroke::new(1.0_f32, stroke);
        widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text_color);
        widgets.hovered.weak_bg_fill = hover_fill;
        widgets.hovered.bg_fill = hover_fill;
        widgets.hovered.bg_stroke = Stroke::new(
            1.0_f32,
            if intent == ButtonIntent::Danger {
                theme::error()
            } else {
                theme::accent()
            },
        );
        widgets.hovered.fg_stroke = Stroke::new(1.0_f32, hover_text);
        widgets.active = widgets.hovered;
        ui.add_enabled(
            enabled,
            egui::Button::new(RichText::new(text).color(text_color).size(13.0))
                .corner_radius(7.0)
                .min_size(egui::vec2(0.0, 26.0)),
        )
    })
    .inner
}

/// A shared, always-visible horizontal scrollbar. Returns the new offset when
/// the user drags or clicks the track.
pub fn horizontal_scroll_bar(ui: &mut Ui, offset: f32, viewport: f32, content: f32) -> Option<f32> {
    let max_offset = (content - viewport).max(0.0);
    if max_offset <= 0.5 {
        return None;
    }
    let width = ui.available_width().max(60.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, 14.0), egui::Sense::click_and_drag());
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 2.0, rect.center().y - 3.0),
        egui::pos2(rect.right() - 2.0, rect.center().y + 3.0),
    );
    ui.painter().rect_filled(track, 3.0, theme::gutter());
    let handle_width = ((viewport / content) * track.width()).clamp(30.0, track.width());
    let travel = (track.width() - handle_width).max(0.0);
    let ratio = (offset / max_offset).clamp(0.0, 1.0);
    let handle = egui::Rect::from_min_size(
        egui::pos2(track.left() + travel * ratio, track.top()),
        egui::vec2(handle_width, track.height()),
    );
    ui.painter().rect_filled(
        handle,
        3.0,
        if response.hovered() || response.dragged() {
            theme::widget_hover()
        } else {
            theme::widget()
        },
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if (response.dragged() || response.clicked()) && travel > 0.0 {
        if let Some(pos) = response.interact_pointer_pos() {
            let delta = (pos.x - track.left() - handle_width / 2.0) / travel;
            ui.ctx().request_repaint();
            return Some(delta.clamp(0.0, 1.0) * max_offset);
        }
    }
    None
}

/// Standard collapse/expand control used by the text, PDF and CSV option bars.
/// Returns `true` when the state was toggled.
pub fn collapse_arrow(ui: &mut Ui, collapsed: &mut bool) -> bool {
    let label = if *collapsed { "▼" } else { "▲" };
    let clicked = intent_button(ui, label, ButtonIntent::Ghost)
        .on_hover_text(if *collapsed {
            "Show comparison options"
        } else {
            "Hide comparison options"
        })
        .clicked();
    if clicked {
        *collapsed = !*collapsed;
    }
    clicked
}

/// Compact `current / total` change counter shared by the text and CSV views.
pub fn change_counter(ui: &mut Ui, current: usize, total: usize) {
    if total == 0 {
        return;
    }
    ui.label(
        RichText::new(format!("{current} / {total}"))
            .color(theme::muted())
            .size(11.5),
    )
    .on_hover_text(format!("{current} of {total} changes"));
}

/// Keep two side-by-side panes on the same scroll offset. The pane under the
/// pointer wins; otherwise the pane whose offset changed last is followed.
pub fn sync_panes(
    sync_x: &mut f32,
    sync_y: &mut f32,
    left_rect: egui::Rect,
    right_rect: egui::Rect,
    left: egui::Vec2,
    right: egui::Vec2,
    pointer: Option<egui::Pos2>,
) {
    if let Some(pointer) = pointer {
        if left_rect.contains(pointer) {
            *sync_x = left.x;
            *sync_y = left.y;
        } else if right_rect.contains(pointer) {
            *sync_x = right.x;
            *sync_y = right.y;
        }
    } else if (left.x - *sync_x).abs() > 0.5 || (left.y - *sync_y).abs() > 0.5 {
        *sync_x = left.x;
        *sync_y = left.y;
    } else if (right.x - *sync_x).abs() > 0.5 || (right.y - *sync_y).abs() > 0.5 {
        *sync_x = right.x;
        *sync_y = right.y;
    }
}

/// Vertical offset that centres a table row in the body viewport. `row_height`
/// excludes the item spacing; `viewport` excludes the header.
pub fn row_scroll_target(row: usize, row_height: f32, item_spacing: f32, viewport: f32) -> f32 {
    let pitch = row_height + item_spacing;
    ((row as f32 + 0.5) * pitch - viewport / 2.0).max(0.0)
}

/// Horizontal offset that centres a character position in a code cell.
pub fn char_scroll_target(char_index: usize, gutter: f32, glyph_width: f32, viewport: f32) -> f32 {
    (gutter + 6.0 + char_index as f32 * glyph_width - viewport / 2.0).max(0.0)
}

pub fn small_muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(theme::muted()).size(11.5));
}

/// Paint the table area as a distinct surface so large dark/light areas do not
/// look like empty void. Call immediately before the comparison table.
pub fn table_surface(ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 20.0 || rect.height() < 20.0 {
        return;
    }
    let painter = ui.painter();
    painter.rect(
        rect,
        8.0,
        theme::panel(),
        Stroke::new(1.0_f32, theme::border()),
        egui::StrokeKind::Inside,
    );
}

/// A soft, full-width card-style button used for recent comparisons and other
/// list-like navigation. Painted explicitly so it matches both themes.
pub fn card_button(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let height = 30.0;
    let width = ui.available_width().max(120.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let hovered = response.hovered();
    let fill = if selected {
        theme::accent_dim()
    } else if hovered {
        theme::widget_hover()
    } else {
        theme::widget()
    };
    let stroke = if hovered || selected {
        theme::accent().gamma_multiply(0.65)
    } else {
        theme::border()
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        8.0,
        fill,
        Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );
    if hovered || selected {
        painter.rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())),
            2.0,
            theme::accent(),
        );
    }
    painter.with_clip_rect(rect).text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        if selected {
            theme::on_selection()
        } else {
            theme::text()
        },
    );
    painter.text(
        egui::pos2(rect.right() - 12.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        "›",
        FontId::proportional(16.0),
        if hovered {
            theme::accent()
        } else {
            theme::faint()
        },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub fn section_heading(ui: &mut Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text)
            .color(theme::muted())
            .size(11.0)
            .strong(),
    );
}

pub fn selectable_chip(
    ui: &mut Ui,
    selected: bool,
    label: impl Into<String>,
    count: usize,
    color: Color32,
) -> Response {
    let text = RichText::new(format!("{}  {}", label.into(), count))
        .color(if selected {
            theme::on_selection()
        } else {
            color
        })
        .size(11.5);
    let response = ui.add(
        egui::Button::new(text)
            .fill(if selected {
                color.gamma_multiply(0.28)
            } else {
                theme::panel_2()
            })
            .stroke(Stroke::new(
                1.0_f32,
                if selected {
                    color.gamma_multiply(0.8)
                } else {
                    theme::border()
                },
            ))
            .corner_radius(10.0),
    );
    response
}
