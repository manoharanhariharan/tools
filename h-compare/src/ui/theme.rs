//! Theme presets, palette derivation and global egui style installation.
//!
//! The presets mirror H-Remote's palette model (primary/accent/background
//! slots) and are adapted into H-Compare's richer diff palette.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    PRemoteM,
    SecretKey,
    Greystone,
    Asphalt,
    Wine,
    Forest,
    Soil,
    HighContrast,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

struct ThemeColors {
    primary_mid: Color32,
    primary_light: Color32,
    primary_dark: Color32,
    primary_text: Color32,
    accent_mid: Color32,
    accent_light: Color32,
    accent_dark: Color32,
    accent_text: Color32,
    background: Color32,
    background_text: Color32,
}

struct ThemePreset {
    mode: ThemeMode,
    display: &'static str,
    colors: ThemeColors,
}

#[allow(clippy::too_many_arguments)]
const fn make_preset(
    mode: ThemeMode,
    display: &'static str,
    primary_mid: u32,
    primary_light: u32,
    primary_dark: u32,
    primary_text: u32,
    accent_mid: u32,
    accent_light: u32,
    accent_dark: u32,
    accent_text: u32,
    background: u32,
    background_text: u32,
) -> ThemePreset {
    ThemePreset {
        mode,
        display,
        colors: ThemeColors {
            primary_mid: rgb(primary_mid),
            primary_light: rgb(primary_light),
            primary_dark: rgb(primary_dark),
            primary_text: rgb(primary_text),
            accent_mid: rgb(accent_mid),
            accent_light: rgb(accent_light),
            accent_dark: rgb(accent_dark),
            accent_text: rgb(accent_text),
            background: rgb(background),
            background_text: rgb(background_text),
        },
    }
}

const PRESETS: [ThemePreset; 10] = [
    make_preset(
        ThemeMode::Dark,
        "Dark",
        0x323233,
        0x474748,
        0x2D2D2D,
        0xCCCCCC,
        0x007ACC,
        0x32A7F4,
        0x0061A3,
        0xFFFFFF,
        0x1E1E1E,
        0xCCCCCC,
    ),
    make_preset(
        ThemeMode::Light,
        "Light",
        0xF2F3F5,
        0xFFFFFF,
        0xE4E7EB,
        0x232323,
        0xE83D61,
        0xED6884,
        0xB5304C,
        0xFFFFFF,
        0xFFFFFF,
        0x000000,
    ),
    make_preset(
        ThemeMode::PRemoteM,
        "PRemoteM",
        0x102B3E,
        0x445A68,
        0x0C2230,
        0xFFFFFF,
        0xE83D61,
        0xED6884,
        0xB5304C,
        0xFFFFFF,
        0xCED8E1,
        0x000000,
    ),
    make_preset(
        ThemeMode::SecretKey,
        "Secret Key",
        0x473368,
        0x796090,
        0x382853,
        0xFFFFFF,
        0xEF6D3B,
        0xFF9A63,
        0xBF572F,
        0xFFFFFF,
        0xF2F1EC,
        0x000000,
    ),
    make_preset(
        ThemeMode::Greystone,
        "Greystone",
        0xC7D0D5,
        0xF9FDFD,
        0x9FA6AA,
        0x1B2C3F,
        0xFF7247,
        0xED583A,
        0xCC5B38,
        0xFFFFFF,
        0xF5F5F5,
        0x000000,
    ),
    make_preset(
        ThemeMode::Asphalt,
        "Asphalt",
        0x393939,
        0x6B6661,
        0x2D2D2D,
        0xFFFFFF,
        0xFF7247,
        0xED583A,
        0xCC5B38,
        0xFFFFFF,
        0xF5F5F5,
        0x000000,
    ),
    make_preset(
        ThemeMode::Wine,
        "Wine",
        0x57112D,
        0x893E55,
        0x450D24,
        0xFFFFFF,
        0xA82159,
        0xDA4E81,
        0x861A47,
        0xFFFFFF,
        0xFDEAD9,
        0x450D24,
    ),
    make_preset(
        ThemeMode::Forest,
        "Forest",
        0x253938,
        0x576660,
        0x1D2D2C,
        0xFFFFFF,
        0x5FA291,
        0x91CFB9,
        0x4C8174,
        0xFFFFFF,
        0xF5F5F5,
        0x303030,
    ),
    make_preset(
        ThemeMode::Soil,
        "Soil",
        0x776245,
        0xA98F6D,
        0x735E41,
        0xFFFFFF,
        0x0193B8,
        0x33C0E0,
        0x007593,
        0xFFFFFF,
        0xCFC3B5,
        0x080000,
    ),
    make_preset(
        ThemeMode::HighContrast,
        "High Contrast",
        0x000000,
        0x1A1A1A,
        0x000000,
        0xFFFFFF,
        0xFFD400,
        0xFFE766,
        0xB89A00,
        0x000000,
        0x000000,
        0xFFFFFF,
    ),
];

impl ThemeMode {
    pub const ALL: [Self; 10] = [
        Self::Dark,
        Self::Light,
        Self::PRemoteM,
        Self::SecretKey,
        Self::Greystone,
        Self::Asphalt,
        Self::Wine,
        Self::Forest,
        Self::Soil,
        Self::HighContrast,
    ];

    pub fn label(self) -> &'static str {
        self.preset().display
    }

    /// Three representative colours for a preset swatch.
    pub fn swatch(self) -> [Color32; 3] {
        let colors = &self.preset().colors;
        let primary = if colors.primary_mid == colors.background {
            colors.primary_light
        } else {
            colors.primary_mid
        };
        [colors.background, primary, colors.accent_mid]
    }

    fn index(self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::PRemoteM => 2,
            Self::SecretKey => 3,
            Self::Greystone => 4,
            Self::Asphalt => 5,
            Self::Wine => 6,
            Self::Forest => 7,
            Self::Soil => 8,
            Self::HighContrast => 9,
        }
    }

    fn preset(self) -> &'static ThemePreset {
        let preset = &PRESETS[self.index()];
        debug_assert_eq!(preset.mode, self);
        preset
    }

    fn is_dark(self) -> bool {
        luminance(self.preset().colors.background) < 0.5
    }
}

static MODE: AtomicU8 = AtomicU8::new(0);
static FONTS_INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn set_mode(mode: ThemeMode) {
    MODE.store(mode.index() as u8, Ordering::Relaxed);
}

pub fn mode() -> ThemeMode {
    ThemeMode::ALL[(MODE.load(Ordering::Relaxed) as usize).min(ThemeMode::ALL.len() - 1)]
}

pub fn is_light() -> bool {
    !mode().is_dark()
}

fn preset() -> &'static ThemePreset {
    mode().preset()
}

fn colors() -> &'static ThemeColors {
    &preset().colors
}

fn dark() -> bool {
    mode().is_dark()
}

fn mix(a: Color32, b: Color32, weight: f32) -> Color32 {
    let t = weight.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 * t + b.r() as f32 * (1.0 - t)).round() as u8,
        (a.g() as f32 * t + b.g() as f32 * (1.0 - t)).round() as u8,
        (a.b() as f32 * t + b.b() as f32 * (1.0 - t)).round() as u8,
    )
}

fn alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn luminance(color: Color32) -> f32 {
    (0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32) / 255.0
}

fn contrast_on(color: Color32) -> Color32 {
    if luminance(color) > 0.55 {
        rgb(0x10141A)
    } else {
        Color32::WHITE
    }
}

pub fn bg() -> Color32 {
    colors().background
}

/// Chrome surfaces: top bar, side panel and status bar.
pub fn panel() -> Color32 {
    let weight = if dark() { 0.88 } else { 0.28 };
    mix(colors().primary_dark, colors().background, weight)
}

/// Cards, menus and table surfaces.
pub fn panel_2() -> Color32 {
    let weight = if dark() { 0.44 } else { 0.14 };
    mix(colors().primary_mid, colors().background, weight)
}

pub fn surface() -> Color32 {
    panel_2()
}

pub fn input() -> Color32 {
    let weight = if dark() { 0.08 } else { 0.04 };
    mix(colors().primary_text, colors().background, weight)
}

pub fn widget() -> Color32 {
    let weight = if dark() { 0.55 } else { 0.14 };
    mix(colors().primary_light, colors().background, weight)
}

pub fn widget_hover() -> Color32 {
    let weight = if dark() { 0.82 } else { 0.24 };
    mix(colors().primary_light, colors().background, weight)
}

pub fn border() -> Color32 {
    alpha(colors().background_text, if dark() { 0.20 } else { 0.14 })
}

pub fn border_light() -> Color32 {
    alpha(colors().background_text, if dark() { 0.36 } else { 0.26 })
}

/// Quiet table grid lines that separate CSV cells without competing with diffs.
pub fn grid() -> Color32 {
    alpha(colors().background_text, if dark() { 0.14 } else { 0.10 })
}

pub fn text() -> Color32 {
    colors().background_text
}

pub fn muted() -> Color32 {
    mix(colors().background_text, colors().background, 0.62)
}

pub fn faint() -> Color32 {
    mix(colors().background_text, colors().background, 0.44)
}

pub fn accent() -> Color32 {
    colors().accent_mid
}

pub fn accent_dim() -> Color32 {
    mix(
        colors().accent_dark,
        colors().background,
        if dark() { 0.28 } else { 0.16 },
    )
}

pub fn accent_light() -> Color32 {
    colors().accent_light
}

pub fn accent_dark() -> Color32 {
    colors().accent_dark
}

pub fn same() -> Color32 {
    if mode() == ThemeMode::HighContrast {
        rgb(0x41D67B)
    } else if dark() {
        rgb(0x58C98A)
    } else {
        rgb(0x1E7A45)
    }
}

pub fn diff() -> Color32 {
    if mode() == ThemeMode::HighContrast {
        rgb(0xFFD400)
    } else if dark() {
        rgb(0xE8B44C)
    } else {
        rgb(0xA36B00)
    }
}

pub fn left() -> Color32 {
    if mode() == ThemeMode::HighContrast {
        rgb(0xFF6B6B)
    } else if dark() {
        rgb(0xE07A7A)
    } else {
        rgb(0xB53030)
    }
}

pub fn right() -> Color32 {
    same()
}

pub fn type_color() -> Color32 {
    if mode() == ThemeMode::HighContrast {
        rgb(0xC9A6FF)
    } else if dark() {
        rgb(0xC39BFF)
    } else {
        rgb(0x6D4AC4)
    }
}

pub fn error() -> Color32 {
    left()
}

pub fn del_bg() -> Color32 {
    mix(
        left(),
        colors().background,
        if dark() { 0.20 } else { 0.10 },
    )
}

pub fn add_bg() -> Color32 {
    mix(
        right(),
        colors().background,
        if dark() { 0.20 } else { 0.10 },
    )
}

pub fn mod_bg() -> Color32 {
    mix(
        diff(),
        colors().background,
        if dark() { 0.20 } else { 0.10 },
    )
}

pub fn sel_bg() -> Color32 {
    accent_dim()
}

pub fn hover_bg() -> Color32 {
    widget_hover()
}

pub fn search_bg() -> Color32 {
    mix(
        diff(),
        colors().background,
        if dark() { 0.58 } else { 0.36 },
    )
}

pub fn del_strong() -> Color32 {
    mix(
        left(),
        colors().background,
        if dark() { 0.40 } else { 0.20 },
    )
}

pub fn add_strong() -> Color32 {
    mix(
        right(),
        colors().background,
        if dark() { 0.40 } else { 0.20 },
    )
}

pub fn gutter() -> Color32 {
    mix(
        colors().primary_dark,
        colors().background,
        if dark() { 0.55 } else { 0.08 },
    )
}

/// Text colour on top of the selection color.
pub fn on_selection() -> Color32 {
    contrast_on(sel_bg())
}

/// Text colour on top of the accent color.
pub fn on_accent() -> Color32 {
    colors().accent_text
}

pub fn on_error() -> Color32 {
    contrast_on(error())
}

pub fn install(ctx: &egui::Context) {
    apply_visuals(ctx);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(9.0, 7.0);
    style.spacing.button_padding = egui::vec2(13.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.menu_margin = egui::Margin::same(6);
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(22.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.5));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.0));
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
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.extreme_bg_color = input();
    visuals.faint_bg_color = panel_2();
    visuals.code_bg_color = gutter();
    visuals.override_text_color = Some(text());
    visuals.hyperlink_color = accent();
    visuals.warn_fg_color = diff();
    visuals.error_fg_color = error();
    visuals.selection.bg_fill = sel_bg();
    visuals.selection.stroke = Stroke::new(1.0_f32, accent_light());
    visuals.widgets.noninteractive.bg_fill = panel();
    visuals.widgets.noninteractive.weak_bg_fill = panel();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);
    visuals.widgets.inactive.bg_fill = widget();
    visuals.widgets.inactive.weak_bg_fill = widget();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border());
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    visuals.widgets.hovered.bg_fill = widget_hover();
    visuals.widgets.hovered.weak_bg_fill = widget_hover();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, accent());
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
    fn every_preset_has_a_readable_background_and_text_pair() {
        let original = mode();
        for theme in ThemeMode::ALL {
            set_mode(theme);
            let difference = (luminance(text()) - luminance(bg())).abs();
            assert!(
                difference > 0.25,
                "{} has low text/background contrast: text {:?} on {:?}",
                theme.label(),
                text(),
                bg()
            );
            assert_eq!(preset().mode, theme);
        }
        set_mode(original);
    }

    #[test]
    fn every_preset_exposes_three_distinct_swatch_colors() {
        for theme in ThemeMode::ALL {
            let [background, primary, accent] = theme.swatch();
            assert_ne!(background, primary, "{} swatch", theme.label());
            assert_ne!(primary, accent, "{} swatch", theme.label());
        }
    }

    #[test]
    fn input_fields_stay_readable_in_every_preset() {
        let original = mode();
        for theme in ThemeMode::ALL {
            set_mode(theme);
            let difference = (luminance(text()) - luminance(input())).abs();
            assert!(
                difference > 0.30,
                "{} input contrast is too low: text {:?} on {:?}",
                theme.label(),
                text(),
                input()
            );
        }
        set_mode(original);
    }

    #[test]
    fn every_preset_round_trips_through_settings_json() {
        for theme in ThemeMode::ALL {
            let encoded = serde_json::to_string(&theme).expect("serialize theme");
            let decoded: ThemeMode = serde_json::from_str(&encoded).expect("deserialize theme");
            assert_eq!(decoded, theme, "theme {encoded}");
        }
    }

    #[test]
    fn both_theme_styles_use_the_active_palette() {
        let ctx = egui::Context::default();
        set_mode(ThemeMode::Dark);
        install(&ctx);
        assert!(ctx.style_of(egui::Theme::Dark).visuals.dark_mode);
        assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, bg());
        assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, bg());
        set_mode(ThemeMode::Light);
        apply_visuals(&ctx);
        assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, bg());
        assert!(!ctx.style_of(egui::Theme::Light).visuals.dark_mode);
        set_mode(ThemeMode::Dark);
        apply_visuals(&ctx);
    }
}
