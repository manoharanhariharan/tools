//! Crisp, theme-aware vector icons drawn directly with the egui painter.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Refresh,
    Swap,
    Search,
    Settings,
    Palette,
    Info,
    Clock,
    More,
    Warning,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    Plus,
    Minus,
    Close,
    Check,
    Download,
    Folder,
    External,
    Restore,
    Undo,
    Columns,
    Layout,
    Copy,
    Trash,
    Sliders,
    Key,
    Filter,
    Eye,
    EyeOff,
}

impl Icon {
    #[cfg(test)]
    pub const ALL: [Self; 31] = [
        Self::Refresh,
        Self::Swap,
        Self::Search,
        Self::Settings,
        Self::Palette,
        Self::Info,
        Self::Clock,
        Self::More,
        Self::Warning,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::ChevronUp,
        Self::ChevronDown,
        Self::Plus,
        Self::Minus,
        Self::Close,
        Self::Check,
        Self::Download,
        Self::Folder,
        Self::External,
        Self::Restore,
        Self::Undo,
        Self::Columns,
        Self::Layout,
        Self::Copy,
        Self::Trash,
        Self::Sliders,
        Self::Key,
        Self::Filter,
        Self::Eye,
        Self::EyeOff,
    ];
}

fn arc_points(center: Pos2, radius: f32, start_deg: f32, end_deg: f32, steps: usize) -> Vec<Pos2> {
    (0..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let angle = (start_deg + (end_deg - start_deg) * t).to_radians();
            Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

/// Paint `icon` inside `rect`, using the rect's smaller side as its size.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 1.0 {
        return;
    }
    let scale = size / 24.0;
    let origin = rect.center() - egui::vec2(12.0 * scale, 12.0 * scale);
    let p = |x: f32, y: f32| origin + egui::vec2(x * scale, y * scale);
    let stroke = Stroke::new((1.65 * scale).clamp(1.1, 2.2), color);
    let thin = Stroke::new((1.35 * scale).clamp(0.9, 1.8), color);
    let dot = |x: f32, y: f32| painter.circle_filled(p(x, y), 1.55 * scale, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let line_thin = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], thin);
    };
    let poly = |points: Vec<(f32, f32)>| {
        painter.add(Shape::line(
            points.into_iter().map(|(x, y)| p(x, y)).collect::<Vec<_>>(),
            stroke,
        ));
    };
    match icon {
        Icon::Refresh => {
            painter.circle_stroke(p(12.0, 12.0), 7.4 * scale, stroke);
            line((16.0, 3.6), (20.2, 6.4));
            line((20.2, 6.4), (15.6, 6.8));
            line((8.0, 20.4), (3.8, 17.6));
            line((3.8, 17.6), (8.4, 17.2));
        }
        Icon::Swap => {
            line((4.0, 9.0), (19.5, 9.0));
            line((4.0, 9.0), (7.3, 5.8));
            line((4.0, 9.0), (7.3, 12.2));
            line((20.0, 15.0), (4.5, 15.0));
            line((20.0, 15.0), (16.7, 11.8));
            line((20.0, 15.0), (16.7, 18.2));
        }
        Icon::Search => {
            painter.circle_stroke(p(10.4, 10.4), 6.2 * scale, stroke);
            line((15.0, 15.0), (20.3, 20.3));
        }
        Icon::Settings => {
            painter.circle_stroke(p(12.0, 12.0), 6.5 * scale, stroke);
            painter.circle_stroke(p(12.0, 12.0), 2.8 * scale, stroke);
            for idx in 0..8 {
                let angle = idx as f32 * std::f32::consts::TAU / 8.0;
                let inner = 6.6;
                let outer = 9.0;
                line(
                    (12.0 + inner * angle.cos(), 12.0 + inner * angle.sin()),
                    (12.0 + outer * angle.cos(), 12.0 + outer * angle.sin()),
                );
            }
        }
        Icon::Palette => {
            painter.circle_stroke(p(12.0, 12.0), 8.2 * scale, stroke);
            dot(8.2, 10.0);
            dot(11.8, 7.6);
            dot(16.0, 10.2);
            dot(15.2, 15.0);
            line_thin((12.6, 15.6), (12.6, 20.0));
        }
        Icon::Info => {
            painter.circle_stroke(p(12.0, 12.0), 8.4 * scale, stroke);
            dot(12.0, 7.7);
            line((12.0, 10.8), (12.0, 16.3));
        }
        Icon::Clock => {
            painter.circle_stroke(p(12.0, 12.0), 8.2 * scale, stroke);
            line((12.0, 7.0), (12.0, 12.2));
            line((12.0, 12.2), (15.6, 14.3));
        }
        Icon::More => {
            dot(5.8, 12.0);
            dot(12.0, 12.0);
            dot(18.2, 12.0);
        }
        Icon::Warning => {
            poly(vec![(12.0, 3.2), (21.6, 20.2), (2.4, 20.2), (12.0, 3.2)]);
            dot(12.0, 15.7);
            line((12.0, 8.5), (12.0, 13.0));
        }
        Icon::ChevronLeft => poly(vec![(15.0, 5.8), (8.8, 12.0), (15.0, 18.2)]),
        Icon::ChevronRight => poly(vec![(9.0, 5.8), (15.2, 12.0), (9.0, 18.2)]),
        Icon::ChevronUp => poly(vec![(5.8, 14.8), (12.0, 8.6), (18.2, 14.8)]),
        Icon::ChevronDown => poly(vec![(5.8, 9.2), (12.0, 15.4), (18.2, 9.2)]),
        Icon::Plus => {
            line((12.0, 4.6), (12.0, 19.4));
            line((4.6, 12.0), (19.4, 12.0));
        }
        Icon::Minus => line((4.6, 12.0), (19.4, 12.0)),
        Icon::Close => {
            line((5.4, 5.4), (18.6, 18.6));
            line((18.6, 5.4), (5.4, 18.6));
        }
        Icon::Check => poly(vec![(4.4, 12.2), (9.6, 17.4), (19.7, 6.8)]),
        Icon::Download => {
            line((12.0, 3.8), (12.0, 15.0));
            poly(vec![(7.8, 10.8), (12.0, 15.0), (16.2, 10.8)]);
            poly(vec![(4.0, 16.8), (4.0, 20.0), (20.0, 20.0), (20.0, 16.8)]);
        }
        Icon::Folder => {
            painter.add(Shape::closed_line(
                vec![
                    p(3.2, 7.0),
                    p(9.0, 7.0),
                    p(11.8, 9.8),
                    p(20.8, 9.8),
                    p(20.8, 19.0),
                    p(3.2, 19.0),
                ],
                stroke,
            ));
            line_thin((3.2, 10.0), (20.8, 10.0));
        }
        Icon::External => {
            poly(vec![(4.0, 8.5), (4.0, 20.0), (15.5, 20.0), (15.5, 13.0)]);
            line((11.0, 13.0), (20.3, 3.7));
            poly(vec![(14.4, 3.7), (20.3, 3.7), (20.3, 9.6)]);
        }
        Icon::Restore => {
            let points = arc_points(p(12.0, 12.0), 7.4 * scale, -40.0, 210.0, 18);
            painter.add(Shape::line(points, stroke));
            line((15.6, 3.8), (20.0, 6.4));
            line((20.0, 6.4), (15.4, 7.0));
        }
        Icon::Undo => {
            let points = arc_points(p(12.0, 12.0), 7.4 * scale, 230.0, 20.0, 18);
            painter.add(Shape::line(points, stroke));
            line((8.4, 3.8), (4.0, 6.4));
            line((4.0, 6.4), (8.6, 7.0));
        }
        Icon::Columns => {
            painter.rect_stroke(
                Rect::from_min_max(p(3.2, 4.2), p(20.8, 19.8)),
                1.2 * scale,
                stroke,
                StrokeKind::Inside,
            );
            line_thin((9.0, 4.2), (9.0, 19.8));
            line_thin((15.0, 4.2), (15.0, 19.8));
        }
        Icon::Layout => {
            painter.rect_stroke(
                Rect::from_min_max(p(3.2, 4.2), p(20.8, 19.8)),
                1.2 * scale,
                stroke,
                StrokeKind::Inside,
            );
            line_thin((12.0, 4.2), (12.0, 19.8));
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(p(8.0, 8.0), p(20.4, 20.4)),
                1.2 * scale,
                stroke,
                StrokeKind::Inside,
            );
            poly(vec![(4.0, 15.6), (4.0, 3.8), (15.8, 3.8)]);
        }
        Icon::Trash => {
            line((3.8, 7.2), (20.2, 7.2));
            poly(vec![(9.0, 7.0), (9.0, 4.0), (15.0, 4.0), (15.0, 7.0)]);
            poly(vec![(5.8, 7.4), (7.0, 20.4), (17.0, 20.4), (18.2, 7.4)]);
            line_thin((10.0, 10.8), (10.3, 17.0));
            line_thin((14.0, 10.8), (13.7, 17.0));
        }
        Icon::Sliders => {
            line((3.0, 7.0), (21.0, 7.0));
            line((3.0, 12.0), (21.0, 12.0));
            line((3.0, 17.0), (21.0, 17.0));
            for (x, y) in [(8.0, 7.0), (15.5, 12.0), (10.5, 17.0)] {
                painter.circle_filled(p(x, y), 2.3 * scale, color);
            }
        }
        Icon::Key => {
            painter.circle_stroke(p(8.0, 16.0), 4.0 * scale, stroke);
            line((11.1, 12.9), (20.0, 4.0));
            line((16.8, 7.2), (19.0, 9.4));
            line((14.6, 9.4), (16.8, 11.6));
        }
        Icon::Filter => {
            poly(vec![
                (3.2, 4.4),
                (20.8, 4.4),
                (14.0, 12.2),
                (14.0, 19.8),
                (10.0, 17.4),
                (10.0, 12.2),
                (3.2, 4.4),
            ]);
        }
        Icon::Eye => {
            poly(vec![
                (2.4, 12.0),
                (6.0, 7.2),
                (12.0, 5.0),
                (18.0, 7.2),
                (21.6, 12.0),
                (18.0, 16.8),
                (12.0, 19.0),
                (6.0, 16.8),
                (2.4, 12.0),
            ]);
            painter.circle_stroke(p(12.0, 12.0), 3.2 * scale, stroke);
        }
        Icon::EyeOff => {
            poly(vec![
                (2.4, 12.0),
                (6.0, 7.2),
                (12.0, 5.0),
                (18.0, 7.2),
                (21.6, 12.0),
                (18.0, 16.8),
                (12.0, 19.0),
                (6.0, 16.8),
                (2.4, 12.0),
            ]);
            painter.circle_stroke(p(12.0, 12.0), 3.2 * scale, stroke);
            line((4.0, 4.0), (20.0, 20.0));
        }
    }
}

/// Paint an icon in a small square at the start of a row.
pub fn small(painter: &Painter, center: Pos2, size: f32, icon: Icon, color: Color32) {
    paint(
        painter,
        Rect::from_center_size(center, egui::vec2(size, size)),
        icon,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_paints_without_panicking() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(360.0, 240.0), egui::Sense::hover());
                let painter = ui.painter();
                for (index, icon) in Icon::ALL.into_iter().enumerate() {
                    let row = index % 8;
                    let column = index / 8;
                    let cell = Rect::from_min_size(
                        rect.min + egui::vec2(column as f32 * 36.0, row as f32 * 36.0),
                        egui::vec2(24.0, 24.0),
                    );
                    paint(painter, cell, icon, Color32::WHITE);
                }
            });
        });
    }
}
