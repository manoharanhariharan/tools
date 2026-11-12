//! Dark/light palettes and global egui style installation.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

static MODE: AtomicU8 = AtomicU8::new(0);
static FONTS_INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn set_mode(mode: ThemeMode) {
    MODE.store(
        match mode {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn mode() -> ThemeMode {
    if is_light() {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

pub fn is_light() -> bool {
    MODE.load(Ordering::Relaxed) == 1
}

fn pick(dark: [u8; 3], light: [u8; 3]) -> Color32 {
    let [r, g, b] = if is_light() { light } else { dark };
    Color32::from_rgb(r, g, b)
}

pub fn bg() -> Color32 {
    pick([16, 18, 22], [246, 247, 250])
}
pub fn panel() -> Color32 {
    pick([22, 25, 31], [255, 255, 255])
}
pub fn panel_2() -> Color32 {
    pick([30, 34, 42], [238, 241, 246])
}
pub fn input() -> Color32 {
    pick([18, 21, 26], [255, 255, 255])
}
pub fn widget() -> Color32 {
    pick([39, 45, 55], [230, 234, 240])
}
pub fn widget_hover() -> Color32 {
    pick([52, 60, 73], [214, 220, 229])
}
pub fn border() -> Color32 {
    pick([57, 65, 78], [201, 208, 218])
}
pub fn border_light() -> Color32 {
    pick([83, 94, 110], [166, 176, 190])
}
pub fn text() -> Color32 {
    pick([232, 236, 243], [28, 34, 42])
}
pub fn muted() -> Color32 {
    pick([151, 161, 176], [91, 102, 117])
}
pub fn faint() -> Color32 {
    pick([104, 114, 129], [138, 148, 162])
}
pub fn accent() -> Color32 {
    pick([96, 170, 255], [22, 104, 199])
}
pub fn accent_dim() -> Color32 {
    pick([31, 66, 105], [207, 226, 251])
}
pub fn same() -> Color32 {
    pick([112, 198, 146], [28, 122, 76])
}
pub fn diff() -> Color32 {
    pick([236, 191, 84], [154, 106, 0])
}
pub fn left() -> Color32 {
    pick([238, 137, 137], [179, 58, 58])
}
pub fn right() -> Color32 {
    pick([107, 208, 151], [24, 130, 82])
}
pub fn type_color() -> Color32 {
    pick([216, 154, 255], [123, 63, 160])
}
pub fn error() -> Color32 {
    pick([255, 120, 120], [179, 38, 30])
}
pub fn del_bg() -> Color32 {
    pick([55, 31, 36], [253, 232, 232])
}
pub fn add_bg() -> Color32 {
    pick([24, 48, 36], [230, 247, 236])
}
pub fn mod_bg() -> Color32 {
    pick([54, 47, 26], [253, 243, 216])
}
pub fn sel_bg() -> Color32 {
    pick([30, 64, 102], [207, 226, 251])
}
pub fn hover_bg() -> Color32 {
    pick([39, 45, 55], [232, 236, 242])
}
pub fn search_bg() -> Color32 {
    pick([124, 100, 24], [255, 229, 138])
}
pub fn del_strong() -> Color32 {
    pick([128, 56, 62], [243, 182, 182])
}
pub fn add_strong() -> Color32 {
    pick([36, 104, 70], [169, 227, 193])
}
pub fn gutter() -> Color32 {
    pick([20, 23, 28], [238, 241, 246])
}

/// Text color on top of the selection color.
pub fn on_selection() -> Color32 {
    if is_light() {
        text()
    } else {
        Color32::WHITE
    }
}

/// Text color on top of the accent color.
pub fn on_accent() -> Color32 {
    if is_light() {
        Color32::WHITE
    } else {
        Color32::from_rgb(10, 22, 34)
    }
}

pub fn on_error() -> Color32 {
    Color32::WHITE
}

pub fn install(ctx: &egui::Context) {
    apply_visuals(ctx);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 26.0;
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(21.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.5));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.5));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.5));
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    install_cjk_fallback(ctx);
}

pub fn apply_visuals(ctx: &egui::Context) {
    let light = is_light();
    let theme = if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    ctx.set_theme(theme);
    let mut visuals = if light {
        Visuals::light()
    } else {
        Visuals::dark()
    };
    visuals.dark_mode = !light;
    visuals.panel_fill = bg();
    visuals.window_fill = panel_2();
    visuals.window_stroke = Stroke::new(1.0_f32, border());
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.extreme_bg_color = input();
    visuals.faint_bg_color = panel_2();
    visuals.code_bg_color = gutter();
    visuals.override_text_color = Some(text());
    visuals.hyperlink_color = accent();
    visuals.warn_fg_color = diff();
    visuals.error_fg_color = error();
    visuals.selection.bg_fill = accent_dim();
    visuals.selection.stroke = Stroke::new(1.0_f32, accent());
    visuals.widgets.noninteractive.bg_fill = panel();
    visuals.widgets.noninteractive.weak_bg_fill = panel();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);
    visuals.widgets.inactive.bg_fill = widget();
    visuals.widgets.inactive.weak_bg_fill = widget();
    visuals.widgets.inactive.bg_stroke = Stroke::new(
        1.0_f32,
        accent().gamma_multiply(if light { 0.55 } else { 0.42 }),
    );
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    visuals.widgets.hovered.bg_fill = widget_hover();
    visuals.widgets.hovered.weak_bg_fill = widget_hover();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.4_f32, accent());
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    visuals.widgets.active.bg_fill = accent_dim();
    visuals.widgets.active.weak_bg_fill = accent_dim();
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent());
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, on_selection());
    visuals.widgets.active.corner_radius = CornerRadius::same(8);
    visuals.widgets.open = visuals.widgets.active;
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.visuals = visuals.clone();
    });
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.visuals = visuals;
    });
}

/// egui's default fonts do not include CJK glyphs. Pick up a system CJK font
/// when one is available so Japanese/Chinese/Korean text is readable.
fn install_cjk_fallback(ctx: &egui::Context) {
    if FONTS_INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }
    use egui::{FontData, FontDefinitions, FontFamily};
    use std::fs;
    use std::sync::Arc;

    let candidates = [
        "C:\\Windows\\Fonts\\YuGothR.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\malgun.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/PingFang.ttc",
    ];
    let mut fonts = FontDefinitions::default();
    let mut added = Vec::new();
    for (idx, path) in candidates.iter().enumerate() {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let name = format!("system_cjk_{idx}");
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
        added.push(name);
        // Keep the fallback small: one Japanese, Chinese and Korean face is
        // usually enough for mixed documents.
        if added.len() >= 3 {
            break;
        }
    }
    if added.is_empty() {
        return;
    }
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            for name in &added {
                list.push(name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
}

pub fn status_color(status: crate::model::EntryStatus) -> Color32 {
    use crate::model::EntryStatus::*;
    match status {
        Same => same(),
        Different => diff(),
        LeftOnly => left(),
        RightOnly => right(),
        TypeMismatch => type_color(),
        Error => error(),
    }
}

pub fn row_bg(kind: crate::textdiff::RowKind, right_side: bool) -> Color32 {
    use crate::textdiff::RowKind::*;
    match (kind, right_side) {
        (LeftOnly, false) => del_bg(),
        (RightOnly, true) => add_bg(),
        (Modified, false) => mod_bg(),
        (Modified, true) => mod_bg(),
        _ => Color32::TRANSPARENT,
    }
}

pub fn cell_bg(kind: crate::csvdiff::CsvRowKind, side_left: bool) -> Color32 {
    use crate::csvdiff::CsvRowKind::*;
    match (kind, side_left) {
        (Modified, true) => del_bg(),
        (Modified, false) => add_bg(),
        (LeftOnly, true) => del_bg(),
        (RightOnly, false) => add_bg(),
        _ => Color32::TRANSPARENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_theme_styles_use_the_active_palette() {
        let ctx = egui::Context::default();
        set_mode(ThemeMode::Dark);
        install(&ctx);
        assert!(ctx.style_of(egui::Theme::Dark).visuals.dark_mode);
        assert!(ctx.style_of(egui::Theme::Light).visuals.dark_mode);
        assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, bg());
        assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, bg());
    }
}
