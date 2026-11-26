//! Side-by-side image comparison with a pixel-difference heatmap.

use crate::image_compare::{ImageCompareData, ImageData};
use crate::ui::common;
use crate::ui::icons::Icon;
use crate::ui::theme;
use eframe::egui::{self, RichText, Sense, TextureHandle, TextureOptions, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    SideBySide,
    Difference,
}

pub struct ImageUiState {
    pub mode: ImageMode,
    pub zoom: f32,
    left_texture: Option<TextureHandle>,
    right_texture: Option<TextureHandle>,
    diff_texture: Option<TextureHandle>,
}

impl Default for ImageUiState {
    fn default() -> Self {
        Self {
            mode: ImageMode::SideBySide,
            zoom: 1.0,
            left_texture: None,
            right_texture: None,
            diff_texture: None,
        }
    }
}

pub fn show(ui: &mut Ui, rel: &str, data: &ImageCompareData, state: &mut ImageUiState) {
    let left_size = data
        .left
        .as_ref()
        .map(|image| format!("{}×{}", image.width, image.height))
        .unwrap_or_else(|| "missing".to_string());
    let right_size = data
        .right
        .as_ref()
        .map(|image| format!("{}×{}", image.width, image.height))
        .unwrap_or_else(|| "missing".to_string());
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(rel).strong().size(14.0));
        ui.label(
            RichText::new("IMAGE")
                .color(theme::type_color())
                .size(10.0)
                .strong(),
        );
        ui.separator();
        common::stat_chip(ui, "A", left_size, theme::left());
        common::stat_chip(ui, "B", right_size, theme::right());
        common::stat_chip(
            ui,
            "changed pixels",
            data.changed_pixels.to_string(),
            theme::diff(),
        );
        if data.left.is_some() && data.right.is_some() {
            let same = data.dimensions_match && data.changed_pixels == 0;
            common::stat_chip(
                ui,
                "status",
                if same {
                    "identical".to_string()
                } else {
                    "different".to_string()
                },
                if same { theme::same() } else { theme::diff() },
            );
        }
    });

    if data.left.is_some() && data.right.is_some() && !data.dimensions_match {
        ui.label(
            RichText::new("Images have different dimensions; the pixel heatmap needs equal sizes.")
                .color(theme::diff())
                .size(11.5),
        );
    }

    ensure_texture(
        ui.ctx(),
        &mut state.left_texture,
        "hcompare_image_left",
        data.left.as_ref(),
    );
    ensure_texture(
        ui.ctx(),
        &mut state.right_texture,
        "hcompare_image_right",
        data.right.as_ref(),
    );
    ensure_texture(
        ui.ctx(),
        &mut state.diff_texture,
        "hcompare_image_diff",
        data.diff.as_ref(),
    );

    ui.horizontal_wrapped(|ui| {
        let mut mode = state.mode;
        common::view_menu(
            ui,
            &mut mode,
            &[
                (ImageMode::SideBySide, "Side by side"),
                (ImageMode::Difference, "Difference"),
            ],
        );
        state.mode = mode;
        if common::icon_button(ui, Icon::Minus, "Zoom out", common::ButtonIntent::Ghost).clicked() {
            state.zoom = (state.zoom / 1.25).clamp(0.25, 8.0);
        }
        ui.label(
            RichText::new(format!("{:.0}%", state.zoom * 100.0))
                .color(theme::muted())
                .size(11.0),
        );
        if common::icon_button(ui, Icon::Plus, "Zoom in", common::ButtonIntent::Ghost).clicked() {
            state.zoom = (state.zoom * 1.25).clamp(0.25, 8.0);
        }
        if common::icon_button(ui, Icon::Refresh, "Fit", common::ButtonIntent::Ghost).clicked() {
            state.zoom = 1.0;
        }
    });

    match state.mode {
        ImageMode::SideBySide => {
            let available = ui.available_size();
            let max_height = (available.y - 12.0).max(80.0);
            let pane_width = ((available.x - 28.0) / 2.0).max(120.0);
            let left_fit = fit_scale(data.left.as_ref(), pane_width, max_height);
            let right_fit = fit_scale(data.right.as_ref(), pane_width, max_height);
            let fit = left_fit.min(right_fit);
            let fit = if fit.is_finite() { fit } else { 1.0 };
            egui::ScrollArea::both()
                .id_salt("image_side_by_side")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        image_pane(
                            ui,
                            "A · LEFT",
                            data.left.as_ref(),
                            state.left_texture.as_ref(),
                            fit,
                            state.zoom,
                        );
                        image_pane(
                            ui,
                            "B · RIGHT",
                            data.right.as_ref(),
                            state.right_texture.as_ref(),
                            fit,
                            state.zoom,
                        );
                    });
                });
        }
        ImageMode::Difference => {
            if let Some(diff) = &data.diff {
                let available = ui.available_size();
                let scale = fit_scale(Some(diff), available.x, (available.y - 24.0).max(80.0))
                    .max(0.01)
                    * state.zoom;
                egui::ScrollArea::both()
                    .id_salt("image_difference")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::NONE
                            .fill(theme::surface())
                            .stroke(egui::Stroke::new(1.0_f32, theme::border()))
                            .corner_radius(8.0)
                            .inner_margin(egui::Margin::same(4))
                            .show(ui, |ui| {
                                if let Some(texture) = &state.diff_texture {
                                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                        texture.id(),
                                        egui::vec2(
                                            diff.width as f32 * scale,
                                            diff.height as f32 * scale,
                                        ),
                                    )));
                                }
                            });
                    });
            } else {
                ui.centered_and_justified(|ui| {
                    common::small_muted(
                        ui,
                        "The difference heatmap is available when both images decode and share the same dimensions.",
                    );
                });
            }
        }
    }

    ui.add_space(4.0);
    common::small_muted(
        ui,
        "Difference heatmap: transparent pixels are equal, orange/red pixels differ. Animated images use the first frame.",
    );
}

fn image_pane(
    ui: &mut Ui,
    label: &str,
    image: Option<&ImageData>,
    texture: Option<&TextureHandle>,
    fit: f32,
    zoom: f32,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .color(theme::muted())
                .size(11.0)
                .strong(),
        );
        ui.add_space(2.0);
        egui::Frame::NONE
            .fill(theme::surface())
            .stroke(egui::Stroke::new(1.0_f32, theme::border()))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                if let (Some(image), Some(texture)) = (image, texture) {
                    let scale = (fit * zoom).max(0.01);
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        egui::vec2(image.width as f32 * scale, image.height as f32 * scale),
                    )));
                } else {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(180.0, 120.0), Sense::hover());
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "missing image",
                        egui::FontId::proportional(12.0),
                        theme::muted(),
                    );
                }
            });
    });
}

fn fit_scale(image: Option<&ImageData>, max_width: f32, max_height: f32) -> f32 {
    let Some(image) = image else {
        return f32::INFINITY;
    };
    if image.width == 0 || image.height == 0 {
        return 1.0;
    }
    (max_width / image.width as f32)
        .min(max_height / image.height as f32)
        .min(1.0)
}

fn ensure_texture(
    ctx: &egui::Context,
    slot: &mut Option<TextureHandle>,
    name: &str,
    data: Option<&ImageData>,
) {
    if slot.is_some() {
        return;
    }
    let Some(data) = data else {
        return;
    };
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [data.width as usize, data.height as usize],
        &data.rgba,
    );
    *slot = Some(ctx.load_texture(name, image, TextureOptions::LINEAR));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, rgba: Vec<u8>) -> ImageData {
        ImageData {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn renders_side_by_side_and_difference_modes() {
        let pixels = vec![255u8; 2 * 2 * 4];
        let data = ImageCompareData {
            left: Some(image(2, 2, pixels.clone())),
            right: Some(image(2, 2, pixels.clone())),
            diff: Some(image(2, 2, vec![0u8; 2 * 2 * 4])),
            changed_pixels: 0,
            dimensions_match: true,
        };
        let mut state = ImageUiState::default();
        let ctx = egui::Context::default();
        for mode in [ImageMode::SideBySide, ImageMode::Difference] {
            state.mode = mode;
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 720.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, "image.png", &data, &mut state);
                });
            });
        }
    }
}
