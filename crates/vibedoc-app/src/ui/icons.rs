//! Line icons drawn with the painter on a 16×16 grid, so they stay crisp at any size.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use super::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    AlignCenter,
    AlignJustify,
    AlignLeft,
    AlignRight,
    Bold,
    BulletList,
    CheckCircle,
    Checklist,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    ClearFormatting,
    Close,
    Code,
    Copy,
    Document,
    File,
    Folder,
    FolderOpen,
    FolderPlus,
    Highlighter,
    Italic,
    LineSpacing,
    Minus,
    Monitor,
    Moon,
    NumberedList,
    Pencil,
    Picture,
    Plus,
    Quote,
    Redo,
    Save,
    Search,
    Send,
    Settings,
    Sidebar,
    Sparkle,
    Strikethrough,
    Subscript,
    Sun,
    Superscript,
    TextColor,
    Underline,
    Undo,
    Warning,
}

/// Paints `icon` centered in `rect`, drawn on a 16×16 grid scaled to fit.
pub fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let scale = rect.width().min(rect.height()) / 16.0;
    let origin = rect.center() - Vec2::splat(8.0 * scale);
    let p = |x: f32, y: f32| origin + vec2(x, y) * scale;
    let stroke = Stroke::new(1.4 * scale, color);
    let line = |points: &[(f32, f32)]| {
        painter.add(Shape::line(points.iter().map(|&(x, y)| p(x, y)).collect(), stroke));
    };
    let outline = |points: &[(f32, f32)]| {
        painter.add(Shape::closed_line(points.iter().map(|&(x, y)| p(x, y)).collect(), stroke));
    };
    let rounded = |min: (f32, f32), max: (f32, f32), radius: f32| {
        let rect = Rect::from_min_max(p(min.0, min.1), p(max.0, max.1));
        painter.rect_stroke(rect, radius * scale, stroke, StrokeKind::Middle);
    };
    let dot = |x: f32, y: f32, r: f32, color: Color32| painter.circle_filled(p(x, y), r * scale, color);
    let arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| {
        let points = (0..=24).map(|k| {
            let t = from + (to - from) * k as f32 / 24.0;
            p(cx + r * t.cos(), cy + r * t.sin())
        });
        painter.add(Shape::line(points.collect(), stroke));
    };
    let letter = |text: &str, format: TextFormat, center: Pos2| {
        let galley = painter.layout_job(LayoutJob::single_section(text.to_string(), format));
        painter.galley(center - galley.size() / 2.0, galley, color);
    };
    let format = |font: FontId| TextFormat { font_id: font, color, ..Default::default() };
    // Lines of text, used by the alignment icons: (left, right) for each of four rows.
    let text_lines = |rows: [(f32, f32); 4]| {
        for (i, (left, right)) in rows.into_iter().enumerate() {
            let y = 3.5 + i as f32 * 3.0;
            line(&[(left, y), (right, y)]);
        }
    };

    match icon {
        Icon::ChevronDown => line(&[(4.5, 6.5), (8.0, 10.0), (11.5, 6.5)]),
        Icon::ChevronUp => line(&[(4.5, 9.5), (8.0, 6.0), (11.5, 9.5)]),
        Icon::ChevronRight => line(&[(6.5, 4.5), (10.0, 8.0), (6.5, 11.5)]),
        Icon::Plus => {
            line(&[(8.0, 3.0), (8.0, 13.0)]);
            line(&[(3.0, 8.0), (13.0, 8.0)]);
        }
        Icon::Minus => line(&[(3.5, 8.0), (12.5, 8.0)]),
        Icon::Close => {
            line(&[(4.0, 4.0), (12.0, 12.0)]);
            line(&[(12.0, 4.0), (4.0, 12.0)]);
        }
        Icon::CheckCircle => {
            dot(8.0, 8.0, 7.0, color);
            let white = Stroke::new(1.6 * scale, Color32::WHITE);
            let check = [(4.8, 8.2), (7.0, 10.4), (11.2, 5.9)].map(|(x, y)| p(x, y));
            painter.add(Shape::line(check.to_vec(), white));
        }
        Icon::Warning => {
            dot(8.0, 8.0, 7.0, color);
            painter.line_segment([p(8.0, 4.3), p(8.0, 9.0)], Stroke::new(1.7 * scale, Color32::WHITE));
            dot(8.0, 11.6, 1.0, Color32::WHITE);
        }
        Icon::Document => {
            outline(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)]);
            line(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)]);
            line(&[(5.8, 8.5), (10.2, 8.5)]);
            line(&[(5.8, 11.2), (8.8, 11.2)]);
        }
        Icon::File => {
            outline(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)]);
            line(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)]);
        }
        Icon::Copy => {
            rounded((5.5, 5.5), (13.5, 13.5), 1.5);
            line(&[(10.5, 5.5), (10.5, 2.5), (2.5, 2.5), (2.5, 10.5), (5.5, 10.5)]);
        }
        Icon::Folder | Icon::FolderPlus => {
            outline(&[(1.5, 3.0), (6.0, 3.0), (7.5, 4.8), (14.5, 4.8), (14.5, 13.5), (1.5, 13.5)]);
            if icon == Icon::FolderPlus {
                line(&[(8.0, 7.0), (8.0, 11.4)]);
                line(&[(5.8, 9.2), (10.2, 9.2)]);
            }
        }
        Icon::FolderOpen => {
            line(&[(2.5, 12.5), (2.5, 3.0), (6.0, 3.0), (7.5, 4.8), (12.5, 4.8), (12.5, 7.0)]);
            outline(&[(2.5, 13.0), (4.5, 7.0), (15.0, 7.0), (13.0, 13.0)]);
        }
        Icon::Search => {
            painter.circle_stroke(p(7.0, 7.0), 4.5 * scale, stroke);
            line(&[(10.4, 10.4), (13.5, 13.5)]);
        }
        Icon::Sparkle => {
            star(painter, p(7.0, 9.0), 6.5 * scale, 1.6 * scale, color);
            star(painter, p(12.8, 3.4), 2.6 * scale, 0.7 * scale, color);
        }
        Icon::Undo => {
            arc(8.5, 9.0, 4.5, PI, TAU + 0.9);
            line(&[(1.8, 6.8), (4.0, 9.4), (6.4, 7.0)]);
        }
        Icon::Redo => {
            arc(7.5, 9.0, 4.5, 0.0, -PI - 0.9);
            line(&[(14.2, 6.8), (12.0, 9.4), (9.6, 7.0)]);
        }
        Icon::Settings => {
            painter.circle_stroke(p(8.0, 8.0), 2.1 * scale, stroke);
            painter.circle_stroke(p(8.0, 8.0), 4.7 * scale, stroke);
            for k in 0..8 {
                let (s, c) = (k as f32 * FRAC_PI_4).sin_cos();
                let tooth = Stroke::new(2.2 * scale, color);
                painter
                    .line_segment([p(8.0 + 4.7 * c, 8.0 + 4.7 * s), p(8.0 + 6.6 * c, 8.0 + 6.6 * s)], tooth);
            }
        }
        Icon::Send => {
            line(&[(8.0, 13.0), (8.0, 3.5)]);
            line(&[(4.0, 7.5), (8.0, 3.5), (12.0, 7.5)]);
        }
        Icon::BulletList | Icon::NumberedList | Icon::Checklist => {
            for (i, y) in [4.0, 8.0, 12.0].into_iter().enumerate() {
                line(&[(7.0, y), (14.5, y)]);
                match icon {
                    Icon::BulletList => {
                        dot(3.0, y, 1.2, color);
                    }
                    Icon::NumberedList => {
                        let font = theme::semibold(5.2 * scale);
                        painter.text(p(3.0, y), Align2::CENTER_CENTER, (i + 1).to_string(), font, color);
                    }
                    _ => {
                        let thin = Stroke::new(1.1 * scale, color);
                        let box_rect = Rect::from_center_size(p(3.2, y), Vec2::splat(3.6 * scale));
                        painter.rect_stroke(box_rect, 0.8 * scale, thin, StrokeKind::Middle);
                    }
                }
            }
        }
        Icon::Quote => {
            for x in [3.0, 9.0] {
                dot(x + 2.0, 6.5, 2.2, color);
                line(&[(x + 4.0, 6.5), (x + 3.2, 10.5), (x + 1.0, 12.5)]);
            }
        }
        Icon::Code => {
            line(&[(5.5, 4.0), (1.8, 8.0), (5.5, 12.0)]);
            line(&[(10.5, 4.0), (14.2, 8.0), (10.5, 12.0)]);
            line(&[(9.2, 2.8), (6.8, 13.2)]);
        }
        Icon::AlignLeft => text_lines([(2.0, 14.0), (2.0, 10.0), (2.0, 14.0), (2.0, 9.0)]),
        Icon::AlignCenter => text_lines([(2.0, 14.0), (4.5, 11.5), (2.0, 14.0), (5.0, 11.0)]),
        Icon::AlignRight => text_lines([(2.0, 14.0), (6.0, 14.0), (2.0, 14.0), (7.0, 14.0)]),
        Icon::AlignJustify => text_lines([(2.0, 14.0), (2.0, 14.0), (2.0, 14.0), (2.0, 10.0)]),
        Icon::LineSpacing => {
            for y in [4.0, 8.0, 12.0] {
                line(&[(7.0, y), (14.5, y)]);
            }
            line(&[(3.0, 2.5), (3.0, 13.5)]);
            line(&[(1.3, 4.5), (3.0, 2.5), (4.7, 4.5)]);
            line(&[(1.3, 11.5), (3.0, 13.5), (4.7, 11.5)]);
        }
        Icon::Save => {
            line(&[(2.5, 10.0), (2.5, 13.5), (13.5, 13.5), (13.5, 10.0)]);
            line(&[(8.0, 2.5), (8.0, 10.0)]);
            line(&[(5.0, 7.0), (8.0, 10.0), (11.0, 7.0)]);
        }
        Icon::Sidebar => {
            rounded((1.5, 2.5), (14.5, 13.5), 2.5);
            line(&[(6.0, 2.5), (6.0, 13.5)]);
        }
        Icon::Picture => {
            rounded((1.5, 3.0), (14.5, 13.0), 2.0);
            line(&[(1.5, 11.5), (6.0, 7.5), (9.0, 10.0), (11.0, 8.5), (14.5, 11.5)]);
            dot(11.0, 5.8, 1.1, color);
        }
        Icon::Pencil => {
            outline(&[(10.5, 2.5), (13.5, 5.5), (5.5, 13.5), (2.5, 13.5), (2.5, 10.5)]);
            line(&[(8.8, 4.2), (11.8, 7.2)]);
        }
        Icon::Sun => {
            painter.circle_stroke(p(8.0, 8.0), 3.0 * scale, stroke);
            for k in 0..8 {
                let (s, c) = (k as f32 * FRAC_PI_4).sin_cos();
                painter
                    .line_segment([p(8.0 + 5.0 * c, 8.0 + 5.0 * s), p(8.0 + 6.8 * c, 8.0 + 6.8 * s)], stroke);
            }
        }
        Icon::Moon => {
            arc(8.0, 8.0, 6.0, -PI * 0.2, PI * 1.3);
            arc(10.6, 5.6, 4.6, PI * 0.42, PI * 1.08);
        }
        Icon::Monitor => {
            rounded((1.5, 2.5), (14.5, 11.0), 1.5);
            line(&[(8.0, 11.0), (8.0, 13.5)]);
            line(&[(5.0, 13.5), (11.0, 13.5)]);
        }
        Icon::Highlighter => {
            outline(&[(9.5, 2.0), (13.5, 6.0), (8.0, 11.5), (4.0, 7.5)]);
            line(&[(4.0, 7.5), (2.5, 11.0), (5.0, 11.0), (8.0, 11.5)]);
        }
        Icon::TextColor => {
            // The bar under the letter is painted by the caller in the chosen color.
            line(&[(3.5, 11.0), (8.0, 1.8), (12.5, 11.0)]);
            line(&[(5.3, 7.5), (10.7, 7.5)]);
        }
        Icon::ClearFormatting => {
            line(&[(3.0, 3.0), (11.0, 3.0)]);
            line(&[(7.0, 3.0), (5.5, 13.0)]);
            line(&[(9.5, 9.5), (14.0, 14.0)]);
            line(&[(14.0, 9.5), (9.5, 14.0)]);
        }
        Icon::Bold => letter("B", format(theme::semibold(13.0 * scale)), rect.center()),
        Icon::Italic => letter(
            "I",
            TextFormat { italics: true, ..format(FontId::proportional(14.0 * scale)) },
            rect.center(),
        ),
        Icon::Underline => letter(
            "U",
            TextFormat {
                underline: Stroke::new(1.2 * scale, color),
                ..format(FontId::proportional(13.0 * scale))
            },
            rect.center(),
        ),
        Icon::Strikethrough => letter(
            "S",
            TextFormat {
                strikethrough: Stroke::new(1.2 * scale, color),
                ..format(FontId::proportional(13.5 * scale))
            },
            rect.center(),
        ),
        Icon::Superscript | Icon::Subscript => {
            letter("x", format(FontId::proportional(13.0 * scale)), p(6.5, 8.5));
            let small = format(theme::semibold(7.5 * scale));
            let y = if icon == Icon::Superscript { 4.0 } else { 12.5 };
            letter("2", small, p(12.5, y));
        }
    }
}

/// A four-pointed star, drawn as four convex kites so it fills cleanly.
fn star(painter: &Painter, center: Pos2, radius: f32, waist: f32, color: Color32) {
    let at = |angle: f32, r: f32| center + vec2(angle.cos(), angle.sin()) * r;
    for k in 0..4 {
        let tip = k as f32 * FRAC_PI_2 - FRAC_PI_2;
        let kite = vec![center, at(tip - FRAC_PI_4, waist), at(tip, radius), at(tip + FRAC_PI_4, waist)];
        painter.add(Shape::convex_polygon(kite, color, Stroke::NONE));
    }
}
